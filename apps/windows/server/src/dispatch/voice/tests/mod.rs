//! 新旧语音协议的生命周期与连接归属回归。

#[cfg(windows)]
mod bridge;
mod global;
mod legacy;
#[cfg(windows)]
mod lifecycle;
mod practice;
