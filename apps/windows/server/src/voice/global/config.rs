//! 全局语音与可选跟读的配置；凭据仅交给翻译工作线程，不参与日志输出。

use qingjian_learning::PracticeMatchConfig;
use qingjian_predict::PredictConfig;

use super::trigger::TriggerConfig;

pub(crate) struct VoiceConfig {
    pub trigger: TriggerConfig,

    pub english_practice: bool,

    pub learn_english: bool,

    pub practice_demo: bool,

    pub practice_match: PracticeMatchConfig,

    pub translation: PredictConfig,
}

impl VoiceConfig {
    pub fn target_language(&self) -> qingjian_core::Language {
        if self.learn_english {
            qingjian_core::Language::English
        } else {
            qingjian_core::Language::Chinese
        }
    }

    pub fn dictation_language(&self) -> qingjian_core::Language {
        if self.english_practice && !self.learn_english {
            qingjian_core::Language::English
        } else {
            qingjian_core::Language::Chinese
        }
    }
}
