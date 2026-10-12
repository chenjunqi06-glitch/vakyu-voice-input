//! 本机累计有效跟读词数决定的三个阶段；只增加经验，不因失败降级。

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PracticeLevel {
    #[default]
    Newcomer,

    Intermediate,

    Challenge,
}

impl PracticeLevel {
    pub const fn from_xp(xp: u64) -> Self {
        match xp {
            0..120 => Self::Newcomer,
            120..500 => Self::Intermediate,
            _ => Self::Challenge,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Newcomer => "新手 Lv.1",
            Self::Intermediate => "进阶 Lv.2",
            Self::Challenge => "挑战 Lv.3",
        }
    }

    pub const fn next_threshold(self) -> Option<u64> {
        match self {
            Self::Newcomer => Some(120),
            Self::Intermediate => Some(500),
            Self::Challenge => None,
        }
    }
}
