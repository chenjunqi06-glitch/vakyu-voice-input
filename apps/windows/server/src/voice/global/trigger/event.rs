//! 语音轮次事件；revision 只表示用户活动发生过，不包含按键内容。

use super::mode::VoiceMode;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TriggerEvent {
    Press {
        sequence: u64,
        revision: u64,
        mode: VoiceMode,
    },
    Held {
        sequence: u64,
        revision: u64,
    },
    Release {
        sequence: u64,
        revision: u64,
    },
    Cancel {
        sequence: u64,
        revision: u64,
    },
}
