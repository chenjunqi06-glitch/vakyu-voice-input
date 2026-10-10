//! 单次暂扣 Alt 的轮次状态。

use super::{input::InputKey, mode::VoiceMode};
use qingjian_platform::PracticeShortcut;
use std::time::Instant;

pub(super) struct Active {
    pub key: InputKey,

    pub sequence: u64,

    pub pressed: Instant,

    pub held: bool,

    pub mode: VoiceMode,

    /// 普通口述没有组合修饰键；此处保存本轮跟读的键，避免两种入口串线。
    pub shortcut: Option<PracticeShortcut>,
}

impl Active {
    pub fn modifiers(&self) -> u8 {
        match self.mode {
            VoiceMode::Dictation => 0,
            VoiceMode::Practice => self.shortcut.map_or(0, |shortcut| shortcut.modifiers),
        }
    }
}
