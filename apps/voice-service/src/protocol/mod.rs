//! 与系统壳和本地识别器的逐行 JSON 协议。

mod asr_event;
mod command;
mod effect;
mod event;
mod mode;

pub use asr_event::AsrEvent;
pub use command::Command;
pub use effect::Effect;
pub use event::Event;
pub use mode::Mode;
