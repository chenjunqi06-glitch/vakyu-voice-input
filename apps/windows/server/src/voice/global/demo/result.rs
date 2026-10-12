//! 一轮示范的最终状态；成功只在音频播放完成后交付。

pub(in crate::voice::global) struct DemoResult {
    pub request_id: u64,

    pub result: Result<(), String>,
}
