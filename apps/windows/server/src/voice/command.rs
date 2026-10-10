//! IPC 线程交给识别线程的控制消息；代次阻止取消后的旧结果串入下一段录音。

use qingjian_core::Language;

pub(crate) enum Command {
    Start(u64),
    StartPractice(u64, String),
    StartInLanguage(u64, Language),
    StartPracticeInLanguage(u64, String, Language),
    Stop(u64),
    Cancel,
    Shutdown,
}

impl Command {
    pub(super) fn with_language_defaults(self) -> Self {
        match self {
            Self::Start(generation) => Self::StartInLanguage(generation, Language::Chinese),
            Self::StartPractice(generation, text) => {
                Self::StartPracticeInLanguage(generation, text, Language::English)
            }
            command => command,
        }
    }
}
