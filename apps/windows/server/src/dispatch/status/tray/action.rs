//! 托盘菜单回到 Router 的操作。

/// 每次点击只修改对应开关，沿用设置页相同的配置键。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    ToggleVoiceInput,

    ToggleEnglishPractice,

    ToggleLearningDirection,

    TogglePracticeDemo,

    /// 「跟读」：开启读完填入原话的跟读；已在这种模式时关闭跟读。
    ToggleShadowing,

    /// 「发英」：开启读完填入译文的跟读；已在这种模式时关闭跟读。
    ToggleSendTranslation,

    ToggleCloud,

    ToggleStatusBar,

    TogglePunctuation,

    OpenSettings,
}
