//! 按住说话的控制与异步结果；桥版本由 InputSettings 协商。

mod action;
mod delivery;
mod update;

pub use action::VoiceAction;
pub use delivery::{VoiceDeliveryCommand, VoiceDeliveryReport};
pub use update::VoiceUpdate;
