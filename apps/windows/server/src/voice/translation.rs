//! 跟读与划词共用的翻译入口：只用随包的本机离线模型，不调用任何网络接口。

use qingjian_core::Language;
use qingjian_predict::reading_target;

use super::offline::OfflineTranslator;

pub(crate) struct Translator {
    local: OfflineTranslator,
}

/// 一轮翻译的结果；失败原因已是给用户看的文字。
pub(crate) struct Translated {
    pub request_id: u64,

    pub result: Result<String, String>,
}

impl Translator {
    /// 跟读用：译文整理成一行。
    pub(crate) fn practice() -> Result<Self, String> {
        OfflineTranslator::new(false).map(|local| Self { local })
    }

    /// 划词用：保留换行，与跟读的实例互不取消。
    pub(crate) fn reading() -> Result<Self, String> {
        OfflineTranslator::new(true).map(|local| Self { local })
    }

    pub(crate) fn request_to(
        &mut self,
        request_id: u64,
        source: &str,
        target: Language,
    ) -> Result<(), String> {
        self.local.request(request_id, source, target)
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
        self.local
            .try_recv()
            .map(|(request_id, result)| Translated { request_id, result })
    }

    pub(crate) fn cancel(&mut self) {
        self.local.cancel();
    }
}
