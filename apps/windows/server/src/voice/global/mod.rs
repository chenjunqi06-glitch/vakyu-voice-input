//! 进程外语音控制：监听系统按键，锁定输入目标，独立于宿主里的 TSF DLL。

mod active;
mod command;
mod config;
mod controller;
mod demo;
mod game;
mod handle;
mod permission;
mod practice;
mod target;
mod trigger;

pub(crate) use config::VoiceConfig;
pub(crate) use handle::GlobalVoice;
pub(crate) use trigger::TriggerConfig;
