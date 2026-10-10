//! 整段译文的异步结果；原文和目标语言由调用方保管。

use super::error::VoicePracticeError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoicePracticeTranslation {
    pub request_id: u64,

    pub result: Result<String, VoicePracticeError>,
}
