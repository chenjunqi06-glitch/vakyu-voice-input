//! Windows 语音按住键；选择单侧时不会占用另一侧 Alt。

use serde::{Deserialize, Serialize};

use super::practice_shortcut::PracticeShortcut;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoiceKey {
    #[default]
    Alt,
    LeftAlt,
    RightAlt,
}

impl VoiceKey {
    /// 两个语音入口必须能独立触发；双侧旧配置会为右侧跟读组合让出右 Alt。
    pub fn conflicts_with_practice(self, shortcut: PracticeShortcut) -> bool {
        if shortcut.is_right_ctrl_alt() {
            return self == Self::RightAlt;
        }
        shortcut.modifiers == 0
            && match self {
                Self::Alt => matches!(shortcut.key, 0xa4 | 0xa5),
                Self::LeftAlt => shortcut.key == 0xa4,
                Self::RightAlt => shortcut.key == 0xa5,
            }
    }
}

#[cfg(test)]
mod tests {
    use super::{PracticeShortcut, VoiceKey};

    #[test]
    fn independent_voice_routes_reject_only_overlapping_triggers() {
        let left_alt = "left_alt".parse::<PracticeShortcut>().unwrap();
        let right_alt = "right_alt".parse::<PracticeShortcut>().unwrap();
        assert!(VoiceKey::LeftAlt.conflicts_with_practice(left_alt));
        assert!(VoiceKey::RightAlt.conflicts_with_practice(right_alt));
        assert!(VoiceKey::Alt.conflicts_with_practice(left_alt));
        assert!(VoiceKey::Alt.conflicts_with_practice(right_alt));
        assert!(!VoiceKey::LeftAlt.conflicts_with_practice(right_alt));
        assert!(!VoiceKey::RightAlt.conflicts_with_practice(left_alt));
        for ordinary in [VoiceKey::LeftAlt, VoiceKey::RightAlt, VoiceKey::Alt] {
            for practice in ["f8", "ctrl+alt+e"] {
                assert!(!ordinary.conflicts_with_practice(practice.parse().unwrap()));
            }
        }
    }

    #[test]
    fn right_control_alt_keeps_left_alt_available() {
        assert!(!VoiceKey::LeftAlt.conflicts_with_practice(PracticeShortcut::RIGHT_CTRL_ALT));
        assert!(!VoiceKey::Alt.conflicts_with_practice(PracticeShortcut::RIGHT_CTRL_ALT));
        assert!(VoiceKey::RightAlt.conflicts_with_practice(PracticeShortcut::RIGHT_CTRL_ALT));
    }
}
