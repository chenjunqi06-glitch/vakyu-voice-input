//! 全局活动的诊断类别；不保存按键值、窗口标识或用户内容。

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u8)]
pub enum ActivitySource {
    #[default]
    None,

    PhysicalKeyboard,

    PhysicalMouse,

    ForeignInjection,

    Foreground,

    FocusNotification,

    Configuration,

    Shutdown,
}

impl ActivitySource {
    pub(super) fn from_raw(value: u8) -> Self {
        match value {
            1 => Self::PhysicalKeyboard,
            2 => Self::PhysicalMouse,
            3 => Self::ForeignInjection,
            4 => Self::Foreground,
            5 => Self::FocusNotification,
            6 => Self::Configuration,
            7 => Self::Shutdown,
            _ => Self::None,
        }
    }
}

pub(super) const fn ignored_while_practicing(source: ActivitySource) -> bool {
    matches!(
        source,
        ActivitySource::PhysicalKeyboard
            | ActivitySource::PhysicalMouse
            | ActivitySource::ForeignInjection
            | ActivitySource::Foreground
            | ActivitySource::FocusNotification
    )
}

pub(super) const fn is_practice_escape(
    practice_active: bool,
    physical: bool,
    virtual_key: u16,
) -> bool {
    practice_active && physical && virtual_key == 0x1b
}
