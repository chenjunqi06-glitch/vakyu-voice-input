//! 跟读与划词翻译用哪种引擎：本机离线翻译（默认，不花钱）或云端 API（需要密钥，按量计费）。

use serde::{Deserialize, Serialize};

/// `[predict] translation_engine`：`local` 本机（Windows 随包离线模型，macOS 系统翻译），`cloud` 走上面的接口。
/// 本机引擎不可用时，若填了密钥就退回云端，否则提示安装或下载语言。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TranslationEngine {
    #[default]
    Local,

    Cloud,
}

impl TranslationEngine {
    pub fn key_string(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Cloud => "cloud",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TranslationEngine;
    use crate::PredictConfig;

    #[test]
    fn local_is_the_default_and_both_values_parse() {
        assert_eq!(
            PredictConfig::default().translation_engine,
            TranslationEngine::Local
        );
        let cloud: PredictConfig =
            serde_json::from_str(r#"{"translation_engine":"cloud"}"#).unwrap();
        assert_eq!(cloud.translation_engine, TranslationEngine::Cloud);
        let legacy: PredictConfig = serde_json::from_str(r#"{"enabled":true}"#).unwrap();
        assert_eq!(legacy.translation_engine, TranslationEngine::Local);
        assert_eq!(TranslationEngine::Cloud.key_string(), "cloud");
    }
}
