//! SAPI 线程循环；停止时先关闭采集，再等已采集音频完成识别。

use super::command::Command;
use super::meter::AudioMeter;
use super::sapi::Sapi;
use super::start::{prepare_and_start, requested};
use super::transcript::Transcript;
use qingjian_platform::protocol::VoiceUpdate;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, atomic::AtomicU64};
use std::time::{Duration, Instant};
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};

const POLL_INTERVAL: Duration = Duration::from_millis(20);
const FINISH_TIMEOUT: Duration = Duration::from_millis(1500);

pub(super) fn run(
    commands: Receiver<Command>,
    updates: Sender<(u64, VoiceUpdate)>,
    desired: Arc<AtomicU64>,
    meter: AudioMeter,
) {
    if let Some(root) = super::neural::runtime_root() {
        super::neural::run(root, commands, updates, desired, meter);
        return;
    }
    tracing::info!("未安装神经网络语音组件，使用 Windows 语音识别");
    if let Err(error) = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok() } {
        while let Ok(command) = commands.recv() {
            match command.with_language_defaults() {
                Command::StartInLanguage(generation, _)
                | Command::StartPracticeInLanguage(generation, _, _) => {
                    let _ = updates.send((
                        generation,
                        VoiceUpdate::Error {
                            message: format!(
                                "Windows 语音初始化失败（0x{:08X}）",
                                error.code().0 as u32
                            ),
                        },
                    ));
                }
                Command::Shutdown => break,
                _ => {}
            }
        }
        return;
    }
    run_initialized(commands, updates, &desired, &meter);
    // run_initialized 已释放全部 COM 对象，再退出 apartment。
    unsafe {
        CoUninitialize();
    }
}

fn run_initialized(
    commands: Receiver<Command>,
    updates: Sender<(u64, VoiceUpdate)>,
    desired: &AtomicU64,
    meter: &AudioMeter,
) {
    let mut sapi: Option<Sapi> = None;
    let mut active: Option<(u64, Option<Instant>)> = None;
    let mut transcript = Transcript::default();
    loop {
        let command = if active.is_some() {
            match commands.recv_timeout(POLL_INTERVAL) {
                Ok(command) => Some(command),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => break,
            }
        } else {
            match commands.recv() {
                Ok(command) => Some(command),
                Err(_) => break,
            }
        };
        match command.map(Command::with_language_defaults) {
            Some(Command::Shutdown) => break,
            Some(Command::Cancel) => {
                // 丢弃音频输入对象，取消后不保留任何麦克风或识别事件引用。
                sapi = None;
                active = None;
                transcript = Transcript::default();
            }
            Some(Command::StartInLanguage(generation, language)) => {
                sapi = None;
                active = None;
                transcript = Transcript::default();
                let started = prepare_and_start(
                    desired,
                    generation,
                    || Sapi::new_in_language(language),
                    Sapi::start,
                    Sapi::cancel,
                );
                match started {
                    Ok(backend) => {
                        sapi = Some(backend);
                        active = Some((generation, None));
                        let _ = updates.send((generation, VoiceUpdate::Started));
                    }
                    Err(Some(update)) => {
                        let _ = updates.send((generation, update));
                    }
                    Err(None) => {}
                }
            }
            Some(Command::StartPracticeInLanguage(generation, _, _)) => {
                let _ = updates.send((
                    generation,
                    VoiceUpdate::Error {
                        message: "跟读需要本机中英识别模型，请修复语音组件".to_owned(),
                    },
                ));
            }
            Some(Command::Stop(generation)) => {
                if let Some((current, deadline)) = &mut active
                    && *current == generation
                    && deadline.is_none()
                {
                    match sapi.as_ref().expect("进行中的识别器").stop() {
                        Ok(()) => *deadline = Some(Instant::now() + FINISH_TIMEOUT),
                        Err(message) => {
                            let _ = updates.send((generation, VoiceUpdate::Error { message }));
                            active = None;
                            sapi = None;
                        }
                    }
                }
            }
            Some(Command::Start(_) | Command::StartPractice(..)) => unreachable!("已补全识别语言"),
            None => {}
        }
        let Some((generation, mut deadline)) = active else {
            continue;
        };
        if requested(desired, generation).is_none() {
            sapi = None;
            active = None;
            continue;
        }
        let recognizer = sapi.as_ref().expect("进行中的识别器");
        if deadline.is_none() && requested(desired, generation) == Some(false) {
            let _ = recognizer.stop();
            deadline = Some(Instant::now() + FINISH_TIMEOUT);
            active = Some((generation, deadline));
        }
        let drained = recognizer.drain(&mut transcript);
        if requested(desired, generation) == Some(true)
            && let Some(level) = recognizer.take_level()
        {
            meter.update(generation, level);
        }
        match drained {
            Ok((changed, ended)) => {
                if ended || deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                    let update = if ended && deadline.is_none() {
                        VoiceUpdate::Error {
                            message: "麦克风音频流提前结束，请检查默认输入设备".to_owned(),
                        }
                    } else {
                        VoiceUpdate::Final {
                            text: transcript.text(),
                        }
                    };
                    active = None;
                    sapi = None;
                    let _ = updates.send((generation, update));
                } else if changed {
                    let _ = updates.send((
                        generation,
                        VoiceUpdate::Interim {
                            text: transcript.text(),
                        },
                    ));
                }
            }
            Err(message) => {
                let _ = updates.send((generation, VoiceUpdate::Error { message }));
                active = None;
                sapi = None;
            }
        }
    }
}
