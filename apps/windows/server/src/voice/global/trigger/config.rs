//! 全局语音触发配置；一次按住使用开始时的阈值。

use qingjian_platform::{PracticeShortcut, VoiceKey};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TriggerConfig {
    pub enabled: bool,

    pub key: VoiceKey,

    pub hold_ms: u32,

    pub practice_enabled: bool,

    pub practice_shortcut: PracticeShortcut,
}

impl TriggerConfig {
    pub(super) fn accepts(self, vk: u16) -> bool {
        if self.practice_enabled {
            return self.enabled
                && if self.practice_shortcut.is_right_ctrl_alt() {
                    matches!(vk, 0xa3 | 0xa5)
                } else {
                    self.practice_shortcut.key == vk
                };
        }
        self.enabled
            && matches!(
                (self.key, vk),
                (VoiceKey::Alt, 0xa4 | 0xa5)
                    | (VoiceKey::LeftAlt, 0xa4)
                    | (VoiceKey::RightAlt, 0xa5)
            )
    }

    pub(super) fn modifiers(self) -> u8 {
        if self.practice_enabled {
            self.practice_shortcut.modifiers
        } else {
            0
        }
    }
}
