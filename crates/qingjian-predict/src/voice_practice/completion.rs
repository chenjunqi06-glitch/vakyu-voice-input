//! 工作线程的带代次回复，不直接暴露给平台调用方。

use super::translation::VoicePracticeTranslation;

#[derive(Debug, Clone)]
pub(super) struct Completion {
    pub generation: u64,

    pub translation: VoicePracticeTranslation,
}
