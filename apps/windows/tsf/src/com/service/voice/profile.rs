//! 语音只服务当前 Miracle 键盘 profile；线程有焦点不代表 Miracle 仍是活动键盘输入法。

use std::cell::RefCell;

use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::UI::TextServices::{
    CLSID_TF_InputProcessorProfiles, GUID_TFCAT_TIP_KEYBOARD, ITfInputProcessorProfileMgr,
    TF_INPUTPROCESSORPROFILE, TF_PROFILETYPE_INPUTPROCESSOR,
};

use crate::com::{CLSID_MIRACLE, GUID_PROFILE};

#[derive(Default)]
pub(super) struct KeyboardProfile {
    manager: RefCell<Option<ITfInputProcessorProfileMgr>>,
}

impl KeyboardProfile {
    /// 仅在新 Alt 边沿和语音会话中查询；COM 接口沿用当前 TSF apartment。
    pub(super) fn active(&self) -> bool {
        let mut manager = self.manager.borrow_mut();
        if manager.is_none() {
            *manager = unsafe {
                CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)
            }
            .ok();
        }
        manager.as_ref().is_some_and(|manager| {
            let mut profile = TF_INPUTPROCESSORPROFILE::default();
            unsafe { manager.GetActiveProfile(&GUID_TFCAT_TIP_KEYBOARD, &mut profile) }.is_ok()
                && is_qingjian(&profile)
        })
    }
}

fn is_qingjian(profile: &TF_INPUTPROCESSORPROFILE) -> bool {
    profile.dwProfileType == TF_PROFILETYPE_INPUTPROCESSOR
        && profile.clsid == CLSID_MIRACLE
        && profile.guidProfile == GUID_PROFILE
}

#[cfg(test)]
mod tests {
    use super::is_qingjian;
    use crate::com::{CLSID_MIRACLE, GUID_PROFILE};
    use windows::Win32::UI::TextServices::{
        TF_INPUTPROCESSORPROFILE, TF_PROFILETYPE_INPUTPROCESSOR, TF_PROFILETYPE_KEYBOARDLAYOUT,
    };
    use windows::core::GUID;

    #[test]
    fn only_the_registered_qingjian_keyboard_profile_can_record() {
        let mut profile = TF_INPUTPROCESSORPROFILE {
            dwProfileType: TF_PROFILETYPE_INPUTPROCESSOR,
            clsid: CLSID_MIRACLE,
            guidProfile: GUID_PROFILE,
            ..Default::default()
        };
        assert!(is_qingjian(&profile));
        profile.clsid = GUID::zeroed();
        assert!(!is_qingjian(&profile));
        profile.clsid = CLSID_MIRACLE;
        profile.guidProfile = GUID::zeroed();
        assert!(!is_qingjian(&profile));
        profile.guidProfile = GUID_PROFILE;
        profile.dwProfileType = TF_PROFILETYPE_KEYBOARDLAYOUT;
        assert!(!is_qingjian(&profile));
    }
}
