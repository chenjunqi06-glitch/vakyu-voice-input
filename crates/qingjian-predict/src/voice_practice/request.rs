//! 待翻译的一轮原文和目标语言；代次独立于调用方序号，避免重用序号时收错结果。

use qingjian_core::Language;

#[derive(Clone)]
pub(super) struct Request {
    pub generation: u64,

    pub request_id: u64,

    pub source: String,

    pub target: Language,
}
