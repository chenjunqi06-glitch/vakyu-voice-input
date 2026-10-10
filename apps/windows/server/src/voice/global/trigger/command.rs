//! 钩子线程命令：配置更新与带持久化回执的快捷键变更。

use std::{path::PathBuf, sync::mpsc::Sender};

use qingjian_platform::PracticeShortcut;

use super::config::TriggerConfig;

pub(super) enum Command {
    Configure(TriggerConfig, Sender<Result<(), String>>),

    SaveShortcut(PracticeShortcut, PathBuf, Sender<Result<(), String>>),
}
