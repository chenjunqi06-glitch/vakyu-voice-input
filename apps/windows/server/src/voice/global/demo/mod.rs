//! 英语示范朗读：专用线程持有播放器（随包自然人声或 SAPI 系统声音），取消后丢弃旧轮次结果。

mod command;
mod handle;
mod result;
mod speaker;
mod worker;

pub(super) use handle::EnglishDemo;
