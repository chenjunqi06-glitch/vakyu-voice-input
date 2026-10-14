//! 按住说话的本地语音后端：优先神经网络识别，未安装时沿用 SAPI。

#[cfg(windows)]
mod command;
#[cfg(windows)]
mod event;
#[cfg(windows)]
pub(crate) mod global;
mod manager;
#[cfg(windows)]
mod meter;
#[cfg(windows)]
mod neural;
#[cfg(windows)]
mod offline;
#[cfg(windows)]
mod sapi;
#[cfg(windows)]
mod start;
#[cfg(any(windows, test))]
mod transcript;
#[cfg(windows)]
pub(crate) mod translation;
#[cfg(windows)]
mod worker;

pub use manager::VoiceManager;

#[cfg(all(test, windows))]
pub(crate) use command::Command as TestVoiceCommand;
