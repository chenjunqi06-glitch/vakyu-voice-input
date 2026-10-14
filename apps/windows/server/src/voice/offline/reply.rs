//! 子进程回来的一条结果，按请求号转给发起请求的翻译器。

pub(super) struct Reply {
    pub wire: u64,

    /// 译文，或给用户看的失败原因。
    pub result: Result<String, String>,
}
