//! 全局语音线程句柄；更新暂停先同步撤销上屏许可，再等待工作线程确认。

use std::sync::Arc;
use std::sync::mpsc::{self, Sender};
use std::time::Duration;

use crate::dispatch::{PracticeAction, VoiceSink};

use super::command::Command;
use super::config::VoiceConfig;
use super::permission::Permission;

pub(crate) struct GlobalVoice {
    commands: Sender<Command>,

    permitted: Arc<Permission>,
}

impl GlobalVoice {
    pub fn spawn(
        sink: Box<dyn VoiceSink>,
        config: VoiceConfig,
        delivery: crate::dispatch::voice::delivery::DeliveryBridge,
    ) -> Result<Self, String> {
        let (commands, receiver) = mpsc::channel();
        let (ready, initialized) = mpsc::sync_channel(1);
        let permitted = Arc::new(Permission::new(config.trigger.enabled));
        let worker_permission = Arc::clone(&permitted);
        std::thread::Builder::new()
            .name("miracle-global-voice".to_owned())
            .spawn(move || {
                super::controller::run(sink, config, receiver, worker_permission, ready, delivery);
            })
            .map_err(|error| format!("无法启动全局语音线程：{error}"))?;
        match initialized.recv_timeout(Duration::from_secs(3)) {
            Ok(Ok(())) => Ok(Self {
                commands,
                permitted,
            }),
            result => {
                permitted.suspend();
                let _ = commands.send(Command::Shutdown);
                Err(match result {
                    Ok(Err(message)) => message,
                    _ => "全局语音初始化超时".to_owned(),
                })
            }
        }
    }

    pub fn configure(&self, config: VoiceConfig) {
        let revision = self.permitted.invalidate();
        let _ = self
            .commands
            .send(Command::Configure(Box::new(config), revision));
    }

    pub fn practice_action(&self, action: PracticeAction) {
        let _ = self.commands.send(Command::PracticeAction(action));
    }

    pub fn save_shortcut(
        &self,
        shortcut: qingjian_platform::PracticeShortcut,
        path: std::path::PathBuf,
    ) -> Result<(), String> {
        let (reply, result) = mpsc::channel();
        let timeout = Duration::from_secs(3);
        let deadline = std::time::Instant::now() + timeout;
        self.commands
            .send(Command::SaveShortcut(shortcut, path, reply, deadline))
            .map_err(|_| "语音后台已退出，快捷键未保存".to_owned())?;
        result
            .recv_timeout(timeout)
            .map_err(|_| "语音后台未及时确认保存，原快捷键保持不变，请稍后重试".to_owned())?
    }

    pub fn prepare_update(&self) -> bool {
        self.permitted.suspend();
        let (reply, confirmed) = mpsc::channel();
        self.commands.send(Command::Pause(reply)).is_ok()
            && confirmed.recv_timeout(Duration::from_millis(1000)).is_ok()
    }
}

impl Drop for GlobalVoice {
    fn drop(&mut self) {
        self.permitted.suspend();
        let _ = self.commands.send(Command::Shutdown);
    }
}
