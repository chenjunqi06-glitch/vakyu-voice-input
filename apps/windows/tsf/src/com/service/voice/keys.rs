//! 分别读取两侧物理 Alt，兼容只报告通用 VK_MENU 状态的键盘驱动。

use qingjian_platform::VoiceKey;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LMENU, VK_MENU, VK_RMENU};

#[derive(Default)]
pub(super) struct AltKeys {
    pub(super) left: bool,

    pub(super) right: bool,

    pub(super) generic: bool,
}

impl AltKeys {
    pub(super) fn read() -> Self {
        Self {
            left: unsafe { GetAsyncKeyState(i32::from(VK_LMENU.0)) < 0 },
            right: unsafe { GetAsyncKeyState(i32::from(VK_RMENU.0)) < 0 },
            generic: unsafe { GetAsyncKeyState(i32::from(VK_MENU.0)) < 0 },
        }
    }

    pub(super) fn down(&self) -> bool {
        self.left || self.right || self.generic
    }

    pub(super) fn selected_down(&self, key: VoiceKey) -> bool {
        match key {
            VoiceKey::Alt => self.down(),
            VoiceKey::LeftAlt => self.left,
            VoiceKey::RightAlt => self.right,
        }
    }

    pub(super) fn matches(key: VoiceKey, vk: u32, extended: bool) -> bool {
        let left = vk == u32::from(VK_LMENU.0) || (vk == u32::from(VK_MENU.0) && !extended);
        let right = vk == u32::from(VK_RMENU.0) || (vk == u32::from(VK_MENU.0) && extended);
        match key {
            VoiceKey::Alt => left || right,
            VoiceKey::LeftAlt => left,
            VoiceKey::RightAlt => right,
        }
    }

    pub(super) fn side(&self) -> &'static str {
        match (self.left, self.right) {
            (true, true) => "左右",
            (true, false) => "左",
            (false, true) => "右",
            (false, false) => "通用",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AltKeys;
    use qingjian_platform::VoiceKey;

    #[test]
    fn side_setting_distinguishes_generic_tsf_events_and_physical_keys() {
        assert!(AltKeys::matches(VoiceKey::RightAlt, 0x12, true));
        assert!(!AltKeys::matches(VoiceKey::RightAlt, 0x12, false));
        assert!(AltKeys::matches(VoiceKey::LeftAlt, 0x12, false));
        assert!(!AltKeys::matches(VoiceKey::LeftAlt, 0xA5, true));
        let mut keys = AltKeys {
            left: true,
            right: true,
            generic: true,
        };
        assert!(keys.selected_down(VoiceKey::RightAlt));
        keys.right = false;
        assert!(!keys.selected_down(VoiceKey::RightAlt));
        assert!(keys.selected_down(VoiceKey::LeftAlt));
        assert!(keys.selected_down(VoiceKey::Alt));
    }
}
