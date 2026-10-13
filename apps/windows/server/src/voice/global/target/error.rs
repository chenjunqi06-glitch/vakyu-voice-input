//! 目标校验错误与不含输入正文的用户提示。

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub(crate) enum TargetError {
    #[error("accessibility could not be initialized")]
    Initialization,

    #[error("no focused editable control")]
    NoEditableTarget,

    #[error("the focused control is a password field")]
    Password,

    #[error("the focused control is read-only")]
    ReadOnly,

    #[error("the original input target has changed")]
    FocusChanged,

    #[error("the input desktop changed or is inaccessible")]
    DesktopChanged,

    #[error("the input target cannot be accessed at this integrity level")]
    Permission,

    #[error("a modifier key is still pressed")]
    ModifierHeld,

    #[error("the voice request was cancelled")]
    Cancelled,

    #[error("the transcript contains control characters")]
    ControlCharacter,

    #[error("the transcript exceeds the supported input batch size")]
    TooLong,

    #[error("only {inserted} of {expected} input events were accepted")]
    Injection { inserted: u32, expected: u32 },

    #[error("the text service did not confirm the insertion")]
    DeliveryUnconfirmed,

    #[error("the text service rejected the insertion")]
    DeliveryRejected,
}

impl TargetError {
    pub(crate) fn user_message(&self) -> &'static str {
        match self {
            Self::Initialization => "系统输入服务暂不可用，请稍后重试",
            Self::NoEditableTarget => "请先点击可以输入文字的输入框；当前控件暂不支持语音填入",
            Self::Password => "密码输入框不启用语音输入",
            Self::ReadOnly => "当前内容只读，请先点击可编辑的输入框",
            Self::FocusChanged | Self::Cancelled => "输入位置已改变，本次语音已取消",
            Self::DesktopChanged => "电脑已锁定或切换到安全界面，本次语音已取消",
            Self::Permission => "当前程序的权限高于 Vakyu，暂时无法填入；请使用普通权限打开该程序",
            Self::ModifierHeld => "请松开 Alt、Ctrl、Shift 和 Windows 键后重新说话",
            Self::ControlCharacter => "识别结果含换行或控制字符，本次未自动填入",
            Self::TooLong => "本次语音过长，请分成几段输入",
            Self::Injection { inserted: 0, .. } => "当前程序未接受语音文字，请确认输入框可以编辑",
            Self::Injection { .. } => "当前程序只接受了部分语音文字，请检查输入框后再试",
            Self::DeliveryUnconfirmed => "输入法没有确认填入结果，请先检查输入框，避免重复输入",
            Self::DeliveryRejected => "原输入框未接受文字，请确认它仍可编辑后重新录音",
        }
    }
}
