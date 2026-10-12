//! 全局语音线程的配置与更新暂停命令。

use std::sync::mpsc::Sender;

use crate::dispatch::PracticeAction;

use super::config::VoiceConfig;

pub(super) enum Command {
    Configure(Box<VoiceConfig>, u64),
    PracticeAction(PracticeAction),
    SaveShortcut(
        qingjian_platform::PracticeShortcut,
        std::path::PathBuf,
        Sender<Result<(), String>>,
    ),
    Pause(Sender<()>),
    Shutdown,
}
