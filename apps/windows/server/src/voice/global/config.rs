//! 全局语音与可选跟读的配置；跟读翻译只用本机离线模型。

use qingjian_learning::PracticeMatchConfig;

use super::trigger::{TriggerConfig, VoiceMode};

pub(crate) struct VoiceConfig {
    pub trigger: TriggerConfig,

    pub english_practice: bool,

    pub learn_english: bool,

    /// 读完后填入译文而不是原话；只影响之后开始的新一轮。
    pub send_translation: bool,

    pub practice_demo: bool,

    /// 英语示范用随包自然人声时的音色编号；`None` 用 Windows 系统声音。
    pub natural_voice: Option<u16>,

    pub practice_match: PracticeMatchConfig,
}

impl VoiceConfig {
    pub fn recording_mode(&self, mode: VoiceMode) -> crate::dispatch::RecordingMode {
        use crate::dispatch::RecordingMode;
        match mode {
            VoiceMode::Dictation => RecordingMode::Dictation,
            VoiceMode::Practice if self.learn_english => RecordingMode::EnglishPractice,
            VoiceMode::Practice => RecordingMode::ChinesePractice,
        }
    }

    pub fn target_language(&self) -> qingjian_core::Language {
        if self.learn_english {
            qingjian_core::Language::English
        } else {
            qingjian_core::Language::Chinese
        }
    }

    pub fn accepts_mode(&self, mode: VoiceMode) -> bool {
        self.trigger.enabled && (mode == VoiceMode::Dictation || self.english_practice)
    }

    pub fn dictation_language(&self, mode: VoiceMode) -> qingjian_core::Language {
        if mode == VoiceMode::Practice && !self.learn_english {
            qingjian_core::Language::English
        } else {
            qingjian_core::Language::Chinese
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qingjian_core::Language;
    use qingjian_platform::{PracticeShortcut, VoiceKey};

    fn config(practice: bool, learn_english: bool) -> VoiceConfig {
        VoiceConfig {
            trigger: TriggerConfig {
                enabled: true,
                key: VoiceKey::LeftAlt,
                hold_ms: 100,
                practice_enabled: practice,
                practice_shortcut: PracticeShortcut::RIGHT_CTRL_ALT,
            },
            english_practice: practice,
            learn_english,
            send_translation: false,
            practice_demo: false,
            natural_voice: None,
            practice_match: PracticeMatchConfig::default(),
        }
    }

    #[test]
    fn ordinary_dictation_stays_chinese_in_both_learning_directions() {
        for practice in [false, true] {
            for learn_english in [false, true] {
                let config = config(practice, learn_english);
                assert!(config.accepts_mode(VoiceMode::Dictation));
                assert_eq!(
                    config.recording_mode(VoiceMode::Dictation),
                    crate::dispatch::RecordingMode::Dictation
                );
                assert_eq!(
                    config.dictation_language(VoiceMode::Dictation),
                    Language::Chinese
                );
            }
        }
    }

    #[test]
    fn practice_uses_source_language_and_requires_its_switch() {
        assert_eq!(
            config(true, true).recording_mode(VoiceMode::Practice),
            crate::dispatch::RecordingMode::EnglishPractice
        );
        assert_eq!(
            config(true, false).recording_mode(VoiceMode::Practice),
            crate::dispatch::RecordingMode::ChinesePractice
        );
        assert_eq!(
            config(true, true).dictation_language(VoiceMode::Practice),
            Language::Chinese
        );
        assert_eq!(
            config(true, false).dictation_language(VoiceMode::Practice),
            Language::English
        );
        assert!(config(true, false).accepts_mode(VoiceMode::Practice));
        assert!(!config(false, false).accepts_mode(VoiceMode::Practice));
        let mut disabled = config(true, true);
        disabled.trigger.enabled = false;
        assert!(!disabled.accepts_mode(VoiceMode::Dictation));
        assert!(!disabled.accepts_mode(VoiceMode::Practice));
    }
}
