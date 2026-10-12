//! 双向跟读逻辑与本机查词；不调用麦克风、网络、TTS 或输入框。

mod action;
pub mod lookup;
mod matcher;
mod phase;
mod session;
mod view;
mod word_card;

pub use action::PracticeAction;
pub use phase::Phase;
pub use session::PracticeSession;
pub use view::PracticeView;
pub use word_card::PracticeWordCard;

#[cfg(test)]
mod tests;
