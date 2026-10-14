//! 单次 Unicode 输入批次；不使用剪贴板，不送回车，不自动重试部分成功。

use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
    KEYEVENTF_UNICODE, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};

use super::GLOBAL_VOICE_INPUT_TAG;
use super::error::TargetError;

const MAX_UTF16_UNITS: usize = 32_768;

pub(super) fn modifiers_released() -> bool {
    [VK_MENU, VK_CONTROL, VK_SHIFT, VK_LWIN, VK_RWIN]
        .into_iter()
        .all(|key| unsafe { GetAsyncKeyState(i32::from(key.0)) } >= 0)
}

pub(super) fn unicode_inputs(text: &str) -> Result<Vec<INPUT>, TargetError> {
    // 换行、Tab 等不能混入用户的聊天或终端输入，拒绝整条而不是暗中改写转写内容。
    if text
        .chars()
        .any(|character| character.is_control() || matches!(character, '\u{2028}' | '\u{2029}'))
    {
        return Err(TargetError::ControlCharacter);
    }
    let units = text.encode_utf16().count();
    if units > MAX_UTF16_UNITS {
        return Err(TargetError::TooLong);
    }
    let mut inputs = Vec::with_capacity(units * 2);
    for unit in text.encode_utf16() {
        for flags in [KEYEVENTF_UNICODE, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP] {
            inputs.push(INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wScan: unit,
                        dwFlags: flags,
                        dwExtraInfo: GLOBAL_VOICE_INPUT_TAG,
                        ..Default::default()
                    },
                },
            });
        }
    }
    Ok(inputs)
}
