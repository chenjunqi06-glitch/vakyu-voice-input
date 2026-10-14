//! 跟读与划词共用的翻译入口：按 `[predict] translation_engine` 选本机离线模型或云端 API。
//! 本机组件不完整时，如果填了 API 密钥就自动改用云端，否则返回安装提示。

use qingjian_core::Language;
use qingjian_predict::{PredictConfig, TranslationEngine, VoicePracticeTranslator, reading_target};

use super::offline::OfflineTranslator;

pub(crate) enum Translator {
    Cloud(VoicePracticeTranslator),

    Local(OfflineTranslator),
}

/// 一轮翻译的结果；失败原因已是给用户看的文字。
pub(crate) struct Translated {
    pub request_id: u64,

    pub result: Result<String, String>,
}

impl Translator {
    /// 跟读用：译文整理成一行。
    pub(crate) fn practice(config: &PredictConfig) -> Result<Self, String> {
        Self::build(config, false)
    }

    /// 划词用：保留换行，与跟读的实例互不取消。
    pub(crate) fn reading(config: &PredictConfig) -> Result<Self, String> {
        Self::build(config, true)
    }

    fn build(config: &PredictConfig, reading: bool) -> Result<Self, String> {
        let cloud = || {
            let translator = if reading {
                VoicePracticeTranslator::new_reading(config)
            } else {
                VoicePracticeTranslator::new(config)
            };
            translator.map(Self::Cloud)
        };
        match config.translation_engine {
            TranslationEngine::Cloud => cloud().map_err(|error| error.user_message().to_owned()),
            TranslationEngine::Local => match OfflineTranslator::new(reading) {
                Ok(local) => Ok(Self::Local(local)),
                Err(local_error) => cloud().map_err(|_| local_error),
            },
        }
    }

    pub(crate) fn is_local(&self) -> bool {
        matches!(self, Self::Local(_))
    }

    pub(crate) fn request_to(
        &mut self,
        request_id: u64,
        source: &str,
        target: Language,
    ) -> Result<(), String> {
        match self {
            Self::Cloud(cloud) => cloud
                .request_to(request_id, source, target)
                .map_err(|error| error.user_message().to_owned()),
            Self::Local(local) => local.request(request_id, source, target),
        }
    }

    /// 划词：自动定方向，返回目标语言。
    pub(crate) fn request_reading(
        &mut self,
        request_id: u64,
        source: &str,
    ) -> Result<Language, String> {
        let target = reading_target(source);
        self.request_to(request_id, source, target)?;
        Ok(target)
    }

    pub(crate) fn try_recv(&mut self) -> Option<Translated> {
        match self {
            Self::Cloud(cloud) => cloud.try_recv().map(|translation| Translated {
                request_id: translation.request_id,
                result: translation
                    .result
                    .map_err(|error| error.user_message().to_owned()),
            }),
            Self::Local(local) => local
                .try_recv()
                .map(|(request_id, result)| Translated { request_id, result }),
        }
    }

    pub(crate) fn cancel(&mut self) {
        match self {
            Self::Cloud(cloud) => cloud.cancel(),
            Self::Local(local) => local.cancel(),
        }
    }
}
