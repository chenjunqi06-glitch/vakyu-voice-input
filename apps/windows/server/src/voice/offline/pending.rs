//! 正在等的一轮离线翻译：子进程请求号、调用方编号与目标语言。

use qingjian_core::Language;

#[derive(Clone, Copy)]
pub(super) struct Pending {
    pub wire: u64,

    pub request_id: u64,

    pub target: Language,
}
