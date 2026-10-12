//! 后台全局按住键：钩子线程只维护按键生命周期，不访问录音、输入框或正文。

mod active;
mod activity;
mod command;
mod config;
mod event;
mod guard;
mod input;
mod machine;
mod native;
mod outcome;
mod registration;
mod runtime;
mod shared;
mod thread;

pub use activity::ActivitySource;
pub use config::TriggerConfig;
pub use event::TriggerEvent;
pub use thread::HookThread;

#[cfg(test)]
mod tests;
