//! 中英跟读与划词翻译的整段翻译：独立异步任务、固定容量通道、取消后不再交付结果。

mod completion;
mod error;
mod prompt;
mod reading;
mod request;
mod translation;
mod translator;
mod worker;

pub use error::VoicePracticeError;
pub use reading::reading_target;
pub use translation::VoicePracticeTranslation;
pub use translator::VoicePracticeTranslator;

#[cfg(test)]
mod tests;
