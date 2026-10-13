//! 状态条一格的动作。

/// 状态条上一格点下去做什么。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StatusAction {
    /// 「中 / 英」：切模式。
    ToggleMode,

    /// 「，。/ ,.」：切全角标点。
    TogglePunctuation,

    /// 「跟读」：开关读完填入原话的跟读练习。
    TogglePractice,

    /// 「发英 / 发中」：开关读完填入译文的跟读练习。
    ToggleSendTranslation,

    /// 齿轮：打开设置程序（UI 线程直接起进程，不经 Router）。
    OpenSettings,
}
