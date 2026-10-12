//! 跟读翻译错误；对用户的文案不携带服务端原始错误或凭据。

use thiserror::Error;

use crate::error::PredictError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum VoicePracticeError {
    #[error("voice practice translation requires an API key")]
    MissingApiKey,

    #[error("voice practice translation requires a valid HTTP API endpoint")]
    InvalidEndpoint,

    #[error("voice practice translation requires a model")]
    MissingModel,

    #[error("voice practice translation target language is not supported")]
    UnsupportedLanguage,

    #[error("voice practice translation source is empty")]
    EmptySource,

    #[error("voice practice translation source exceeds the supported length")]
    SourceTooLong,

    #[error("voice practice translation reply is invalid")]
    InvalidTranslation,

    #[error("voice practice translation reply is incomplete")]
    IncompleteTranslation,

    #[error("voice practice translation timed out")]
    TimedOut,

    #[error("voice practice translation request failed")]
    RequestFailed,

    #[error("voice practice translation worker is unavailable")]
    Unavailable,
}

impl VoicePracticeError {
    pub fn user_message(self) -> &'static str {
        match self {
            Self::MissingApiKey => "请先在云服务设置中填写 API 密钥，再使用跟读。",
            Self::InvalidEndpoint => "云服务地址无效，请检查云服务设置。",
            Self::MissingModel => "请先在云服务设置中填写模型名称。",
            Self::UnsupportedLanguage => "跟读翻译目前支持中文和英文，请重新选择学习语言。",
            Self::EmptySource => "没有识别到需要翻译的内容，请重新录音。",
            Self::SourceTooLong => "这段内容超过 4000 字，请分次录音。",
            Self::InvalidTranslation => "未取得所选语言的有效译文，请重试。",
            Self::IncompleteTranslation => "译文不完整，已停止本次跟读，请重试。",
            Self::TimedOut => "翻译等待超时，请检查网络后重试。",
            Self::RequestFailed => "翻译失败，请检查云服务连接或稍后重试。",
            Self::Unavailable => "翻译服务暂时不可用，请重试。",
        }
    }
}

impl From<PredictError> for VoicePracticeError {
    fn from(error: PredictError) -> Self {
        match error {
            PredictError::MissingApiKey(_) => Self::MissingApiKey,
            PredictError::Runtime(_) => Self::Unavailable,
            PredictError::Timeout(_) => Self::TimedOut,
            PredictError::EmptyReply | PredictError::Encode(_) => Self::InvalidTranslation,
            PredictError::IncompleteReply | PredictError::BudgetExhausted => {
                Self::IncompleteTranslation
            }
            PredictError::Api(_) => Self::RequestFailed,
        }
    }
}
