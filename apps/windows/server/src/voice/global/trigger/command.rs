//! 钩子线程命令：配置更新与带持久化回执的快捷键变更。

use std::{path::PathBuf, sync::mpsc::Sender, time::Instant};

use qingjian_platform::PracticeShortcut;

use super::config::TriggerConfig;

pub(super) enum Command {
    Configure(TriggerConfig, Sender<Result<(), String>>),

    /// 最后一项是调用方等待的截止时间：过了它调用方已按失败处理，钩子线程不能再晚一步改注册与配置。
    SaveShortcut(
        PracticeShortcut,
        PathBuf,
        Sender<Result<(), String>>,
        Instant,
    ),
}
