//! 英语示范朗读：专用线程持有 SAPI 播放器，取消后丢弃旧轮次结果。

mod command;
mod handle;
mod result;
mod speaker;
mod worker;

pub(super) use handle::EnglishDemo;
