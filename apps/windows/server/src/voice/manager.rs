//! 语音控制入口；开始、停止只入队，最终结果从下一次轮询取得。

#[cfg(windows)]
use super::command::Command;
use qingjian_core::Language;
use qingjian_platform::protocol::VoiceUpdate;
#[cfg(windows)]
use std::sync::Arc;
#[cfg(windows)]
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(windows)]
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};

#[derive(Default)]
pub struct VoiceManager {
    #[cfg(windows)]
    commands: Option<Sender<Command>>,

    #[cfg(windows)]
    updates: Option<Receiver<(u64, VoiceUpdate)>>,

    #[cfg(windows)]
    generation: u64,

    #[cfg(windows)]
    active: bool,

    #[cfg(windows)]
    latest: String,

    #[cfg(windows)]
    desired: Arc<AtomicU64>,

    #[cfg(windows)]
    meter: super::meter::AudioMeter,
}

impl VoiceManager {
    /// 测试用真实控制通道替代音频线程，验证 Router 发出的命令而不访问麦克风。
    #[cfg(all(test, windows))]
    pub(crate) fn test_channels(&mut self) -> (Receiver<Command>, Sender<(u64, VoiceUpdate)>) {
        let (command_tx, command_rx) = mpsc::channel();
        let (update_tx, update_rx) = mpsc::channel();
        self.commands = Some(command_tx);
        self.updates = Some(update_rx);
        (command_rx, update_tx)
    }

    /// 登录启动时只预热模型；默认构造和测试均不会创建录音进程。
    pub fn prewarm(&mut self) {
        #[cfg(windows)]
        if super::neural::runtime_root().is_some()
            && let Err(message) = self.ensure_worker()
        {
            tracing::warn!(%message, "语音预热线程启动失败");
        }
    }

    #[cfg(windows)]
    fn ensure_worker(&mut self) -> Result<(), String> {
        if self.commands.is_some() {
            return Ok(());
        }
        let (command_tx, command_rx) = mpsc::channel();
        let (update_tx, update_rx) = mpsc::channel();
        let desired = Arc::clone(&self.desired);
        let meter = self.meter.clone();
        std::thread::Builder::new()
            .name("miracle-voice".to_owned())
            .spawn(move || super::worker::run(command_rx, update_tx, desired, meter))
            .map_err(|error| format!("无法启动语音线程：{error}"))?;
        self.commands = Some(command_tx);
        self.updates = Some(update_rx);
        Ok(())
    }

    pub fn start(&mut self) -> VoiceUpdate {
        self.start_in_language(Language::Chinese)
    }

    pub fn start_in_language(&mut self, language: Language) -> VoiceUpdate {
        self.start_with_practice(None, language)
    }

    /// 当前麦克风采样得到的音量；松键、取消、旧代次或断流时返回零。
    pub fn level(&self) -> u8 {
        #[cfg(windows)]
        if self.active && super::start::requested(&self.desired, self.generation) == Some(true) {
            return self.meter.level(self.generation);
        }
        0
    }

    /// 译句只用于本轮本机识别提示；普通口述不会继承上一轮练习内容。
    #[cfg(test)]
    pub fn start_practice(&mut self, expected: &str) -> VoiceUpdate {
        self.start_practice_in_language(expected, Language::English)
    }

    pub fn start_practice_in_language(
        &mut self,
        expected: &str,
        language: Language,
    ) -> VoiceUpdate {
        self.start_with_practice(Some(expected.to_owned()), language)
    }

    fn start_with_practice(
        &mut self,
        practice_text: Option<String>,
        language: Language,
    ) -> VoiceUpdate {
        if !matches!(language, Language::Chinese | Language::English) {
            return VoiceUpdate::Error {
                message: "本机语音组件目前支持中文和英语".to_owned(),
            };
        }
        #[cfg(not(windows))]
        {
            let _ = practice_text;
            VoiceUpdate::Error {
                message: "语音输入仅支持 Windows".to_owned(),
            }
        }
        #[cfg(windows)]
        {
            self.cancel();
            self.desired
                .store((self.generation << 1) | 1, Ordering::Release);
            if let Err(message) = self.ensure_worker() {
                self.desired.store(self.generation << 1, Ordering::Release);
                return VoiceUpdate::Error { message };
            }
            let command = match (practice_text, language) {
                (Some(text), Language::English) => Command::StartPractice(self.generation, text),
                (None, Language::Chinese) => Command::Start(self.generation),
                (Some(text), language) => {
                    Command::StartPracticeInLanguage(self.generation, text, language)
                }
                (None, language) => Command::StartInLanguage(self.generation, language),
            };
            if self
                .commands
                .as_ref()
                .is_some_and(|tx| tx.send(command).is_ok())
            {
                self.active = true;
                // 先确认请求已接收；模型或音频初始化失败时在 Poll 返回明确错误。
                VoiceUpdate::Starting
            } else {
                self.desired.store(self.generation << 1, Ordering::Release);
                self.commands = None;
                self.updates = None;
                VoiceUpdate::Error {
                    message: "语音线程已退出，请再次按住语音键重试".to_owned(),
                }
            }
        }
    }

    pub fn poll(&mut self) -> Option<VoiceUpdate> {
        #[cfg(not(windows))]
        {
            None
        }
        #[cfg(windows)]
        {
            let rx = self.updates.as_ref()?;
            let mut latest_update = None;
            loop {
                match rx.try_recv() {
                    Ok((generation, update)) if generation == self.generation && self.active => {
                        if update == VoiceUpdate::Started
                            && super::start::requested(&self.desired, generation) != Some(true)
                        {
                            continue;
                        }
                        if let VoiceUpdate::Interim { text } | VoiceUpdate::Final { text } = &update
                        {
                            self.latest.clone_from(text);
                        }
                        if matches!(
                            update,
                            VoiceUpdate::Final { .. } | VoiceUpdate::Error { .. }
                        ) {
                            self.active = false;
                        }
                        latest_update = Some(update);
                    }
                    Ok(_) => {}
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        if self.active {
                            self.active = false;
                            latest_update = Some(VoiceUpdate::Error {
                                message: "语音识别意外退出，请检查麦克风后重试".to_owned(),
                            });
                        }
                        self.commands = None;
                        self.updates = None;
                        break;
                    }
                }
            }
            latest_update
        }
    }

    /// 停止采集后处理已经收到的音频；不会阻塞键盘线程。
    pub fn stop(&mut self) -> VoiceUpdate {
        #[cfg(not(windows))]
        {
            VoiceUpdate::Error {
                message: "语音输入仅支持 Windows".to_owned(),
            }
        }
        #[cfg(windows)]
        {
            // 不能等 Stop 消息排到语音线程才撤销开麦意图：此时可能还在加载模型。
            self.desired.store(self.generation << 1, Ordering::Release);
            if let Some(update @ (VoiceUpdate::Final { .. } | VoiceUpdate::Error { .. })) =
                self.poll()
            {
                return update;
            }
            if !self.active {
                return VoiceUpdate::Final {
                    text: self.latest.clone(),
                };
            }
            if self
                .commands
                .as_ref()
                .is_some_and(|tx| tx.send(Command::Stop(self.generation)).is_ok())
            {
                VoiceUpdate::Stopping
            } else {
                self.active = false;
                VoiceUpdate::Error {
                    message: "语音识别已断开".to_owned(),
                }
            }
        }
    }

    pub fn cancel(&mut self) {
        #[cfg(windows)]
        {
            self.generation = self.generation.wrapping_add(1) & (u64::MAX >> 1);
            self.desired.store(self.generation << 1, Ordering::Release);
            self.active = false;
            self.latest.clear();
            if let Some(tx) = &self.commands {
                let _ = tx.send(Command::Cancel);
            }
        }
    }
}

impl Drop for VoiceManager {
    fn drop(&mut self) {
        #[cfg(windows)]
        {
            self.desired
                .store(self.generation.wrapping_add(1) << 1, Ordering::Release);
            if let Some(tx) = &self.commands {
                let _ = tx.send(Command::Shutdown);
            }
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::VoiceManager;
    use crate::voice::command::Command;
    use crate::voice::start::requested;
    use qingjian_core::Language;
    use qingjian_platform::protocol::VoiceUpdate;
    use std::sync::mpsc;

    #[test]
    fn reverse_practice_sends_english_source_then_chinese_practice() {
        let mut manager = VoiceManager::default();
        let (commands, _updates) = manager.test_channels();
        assert_eq!(
            manager.start_in_language(Language::English),
            VoiceUpdate::Starting
        );
        assert!(matches!(commands.recv().unwrap(), Command::Cancel));
        assert!(matches!(
            commands.recv().unwrap(),
            Command::StartInLanguage(1, Language::English)
        ));
        assert_eq!(
            manager.start_practice_in_language("我今天去吃饭", Language::Chinese),
            VoiceUpdate::Starting
        );
        assert!(matches!(commands.recv().unwrap(), Command::Cancel));
        assert!(
            matches!(commands.recv().unwrap(), Command::StartPracticeInLanguage(2, text, Language::Chinese) if text == "我今天去吃饭")
        );
        assert_eq!(manager.start(), VoiceUpdate::Starting);
        assert!(matches!(commands.recv().unwrap(), Command::Cancel));
        assert!(matches!(commands.recv().unwrap(), Command::Start(3)));
    }

    #[test]
    fn unsupported_language_does_not_start_microphone() {
        let mut manager = VoiceManager::default();
        let (commands, _updates) = manager.test_channels();
        assert!(matches!(
            manager.start_in_language(Language::Japanese),
            VoiceUpdate::Error { .. }
        ));
        assert!(commands.try_recv().is_err());
    }

    #[test]
    fn starting_does_not_report_ready_and_release_is_immediate() {
        // 保留消息通道但不启动工人，模拟工人仍在进行耗时的初始化。
        let (command_tx, command_rx) = mpsc::channel();
        let (update_tx, update_rx) = mpsc::channel();
        let mut manager = VoiceManager {
            commands: Some(command_tx),
            updates: Some(update_rx),
            generation: 0,
            active: false,
            latest: String::new(),
            desired: Default::default(),
            meter: Default::default(),
        };
        assert_eq!(manager.start(), VoiceUpdate::Starting);
        assert_eq!(requested(&manager.desired, manager.generation), Some(true));
        manager.meter.update(manager.generation, 47);
        assert_eq!(manager.level(), 47);
        assert_eq!(manager.poll(), None);
        assert_eq!(manager.stop(), VoiceUpdate::Stopping);
        assert_eq!(requested(&manager.desired, manager.generation), Some(false));
        manager.meter.update(manager.generation, 82);
        assert_eq!(manager.level(), 0, "松键后到达的旧音量不能再显示");
        assert!(matches!(command_rx.recv().unwrap(), Command::Cancel));
        assert!(matches!(command_rx.recv().unwrap(), Command::Start(1)));
        assert!(matches!(command_rx.recv().unwrap(), Command::Stop(1)));
        update_tx.send((1, VoiceUpdate::Started)).unwrap();
        assert_eq!(
            manager.poll(),
            None,
            "松键后的迟到 Started 不能重新显示正在录音"
        );
        update_tx
            .send((
                1,
                VoiceUpdate::Final {
                    text: String::new(),
                },
            ))
            .unwrap();
        assert_eq!(
            manager.poll(),
            Some(VoiceUpdate::Final {
                text: String::new()
            })
        );
    }

    #[test]
    fn cancelling_invalidates_a_start_before_worker_processes_its_queue() {
        let (command_tx, _command_rx) = mpsc::channel();
        let (_update_tx, update_rx) = mpsc::channel();
        let mut manager = VoiceManager {
            commands: Some(command_tx),
            updates: Some(update_rx),
            generation: 0,
            active: false,
            latest: String::new(),
            desired: Default::default(),
            meter: Default::default(),
        };
        manager.start();
        let generation = manager.generation;
        manager.cancel();
        assert_eq!(requested(&manager.desired, generation), None);
    }

    #[test]
    fn practice_cancel_then_ordinary_start_does_not_reuse_expected_text() {
        let mut manager = VoiceManager::default();
        let (commands, updates) = manager.test_channels();
        assert_eq!(
            manager.start_practice("My name is Chen Junqi."),
            VoiceUpdate::Starting
        );
        let practice_generation = manager.generation;
        assert!(matches!(commands.recv().unwrap(), Command::Cancel));
        assert!(matches!(
            commands.recv().unwrap(),
            Command::StartPractice(generation, text)
                if generation == practice_generation && text == "My name is Chen Junqi."
        ));
        manager.cancel();
        assert_eq!(requested(&manager.desired, practice_generation), None);
        assert_eq!(manager.start(), VoiceUpdate::Starting);
        manager.meter.update(practice_generation, 90);
        assert_eq!(manager.level(), 0, "新录音不能显示上一轮音量");
        assert!(matches!(commands.recv().unwrap(), Command::Cancel));
        assert!(matches!(commands.recv().unwrap(), Command::Cancel));
        assert!(matches!(
            commands.recv().unwrap(),
            Command::Start(generation) if generation == manager.generation
        ));
        updates
            .send((
                practice_generation,
                VoiceUpdate::Final {
                    text: "My name is Chen Junqi.".to_owned(),
                },
            ))
            .unwrap();
        assert_eq!(manager.poll(), None);
        assert!(manager.latest.is_empty());
        assert!(manager.active);
    }

    #[test]
    fn stopping_waits_for_final_without_losing_accumulated_text() {
        let (command_tx, command_rx) = mpsc::channel();
        let (update_tx, update_rx) = mpsc::channel();
        let mut manager = VoiceManager {
            commands: Some(command_tx),
            updates: Some(update_rx),
            generation: 4,
            active: true,
            latest: String::new(),
            desired: Default::default(),
            meter: Default::default(),
        };
        update_tx
            .send((
                4,
                VoiceUpdate::Interim {
                    text: "今天去吃饭".to_owned(),
                },
            ))
            .unwrap();
        assert_eq!(manager.stop(), VoiceUpdate::Stopping);
        assert!(matches!(command_rx.recv().unwrap(), Command::Stop(4)));
        update_tx
            .send((
                4,
                VoiceUpdate::Final {
                    text: "今天去吃饭。".to_owned(),
                },
            ))
            .unwrap();
        assert_eq!(
            manager.poll(),
            Some(VoiceUpdate::Final {
                text: "今天去吃饭。".to_owned()
            })
        );
        assert!(!manager.active);
    }

    #[test]
    fn cancelled_results_cannot_enter_a_new_recording() {
        let (command_tx, _command_rx) = mpsc::channel();
        let (update_tx, update_rx) = mpsc::channel();
        let mut manager = VoiceManager {
            commands: Some(command_tx),
            updates: Some(update_rx),
            generation: 7,
            active: true,
            latest: String::new(),
            desired: Default::default(),
            meter: Default::default(),
        };
        manager.cancel();
        manager.active = true;
        update_tx
            .send((
                7,
                VoiceUpdate::Final {
                    text: "旧录音".to_owned(),
                },
            ))
            .unwrap();
        update_tx
            .send((
                8,
                VoiceUpdate::Interim {
                    text: "新录音".to_owned(),
                },
            ))
            .unwrap();
        assert_eq!(
            manager.poll(),
            Some(VoiceUpdate::Interim {
                text: "新录音".to_owned()
            })
        );
    }
}
