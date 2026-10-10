//! 单次语音代次与截止时间；初始化完成不代表仍然允许打开麦克风。

use qingjian_core::Language;
use std::time::{Duration, Instant};

use super::event::HelperEvent;

pub(super) const START_TIMEOUT: Duration = Duration::from_secs(5);
pub(super) const FINISH_TIMEOUT: Duration = Duration::from_secs(12);

pub(super) struct Session {
    pub generation: u64,

    pub practice_text: Option<String>,

    pub language: Language,

    pub dispatched: Option<Instant>,

    pub started: bool,

    pub finishing: Option<Instant>,
}

impl Session {
    #[cfg(test)]
    pub fn new(generation: u64) -> Self {
        Self::in_language(generation, Language::Chinese)
    }

    pub fn in_language(generation: u64, language: Language) -> Self {
        Self {
            generation,
            language,
            practice_text: None,
            dispatched: None,
            started: false,
            finishing: None,
        }
    }

    #[cfg(test)]
    pub fn practice(generation: u64, text: String) -> Self {
        Self::practice_in_language(generation, text, Language::English)
    }

    pub fn practice_in_language(generation: u64, text: String, language: Language) -> Self {
        Self {
            practice_text: Some(text),
            ..Self::in_language(generation, language)
        }
    }

    pub fn accepts(&self, event: &HelperEvent) -> bool {
        event.generation() == Some(self.generation) && self.dispatched.is_some()
    }

    pub fn timeout(&self, now: Instant) -> Option<&'static str> {
        if self.finishing.is_some_and(|deadline| now >= deadline) {
            Some("语音结果处理超时，请重新按住 Alt 录音")
        } else if !self.started
            && self.finishing.is_none()
            && self
                .dispatched
                .is_some_and(|sent| now.duration_since(sent) >= START_TIMEOUT)
        {
            Some("麦克风启动超时，请检查默认输入设备")
        } else {
            None
        }
    }
}
