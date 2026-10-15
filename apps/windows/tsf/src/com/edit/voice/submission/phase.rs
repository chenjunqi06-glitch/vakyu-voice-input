//! 单次语音提交的排队与执行阶段；执行中不能再排入同一请求。

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum Phase {
    #[default]
    Idle,

    Queued,

    Writing,

    Committed,
}
