//! 常驻中英语音识别进程；预热不打开麦克风，按住会话才发出录音命令。

mod closing;
mod command;
mod event;
mod process;
mod runtime;
mod session;
mod worker;

#[cfg(test)]
mod tests;

use std::path::PathBuf;

pub(super) use worker::run;

pub(super) fn runtime_root() -> Option<PathBuf> {
    let executable = std::env::current_exe().ok();
    let local_data = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    runtime::select_root(executable.as_deref(), local_data.as_deref())
}
