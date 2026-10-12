//! 托盘菜单回到 Router 的操作。

/// 每次点击只修改对应开关，沿用设置页相同的配置键。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    ToggleVoiceInput,

    ToggleEnglishPractice,

    ToggleLearningDirection,

    TogglePracticeDemo,

    ToggleCloud,

    ToggleStatusBar,

    TogglePunctuation,

    OpenSettings,
}
