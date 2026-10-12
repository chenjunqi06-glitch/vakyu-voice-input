//! 跟读界面快照复用跨平台定义；Windows 消息保留现有操作编码。

pub use qingjian_practice::{PracticeView, PracticeWordCard};

/// 英语跟读浮窗上的显式操作。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PracticeAction {
    LookupWord(usize),

    SpeakWord,

    CloseWordCard,

    Retry,
    SkipCurrent,
    AcceptCurrent,
    Cancel,
}
