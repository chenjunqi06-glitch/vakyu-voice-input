//! 编辑目标策略与 Unicode 事件的离线验证；不触发系统输入或打开应用。

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Accessibility::ROLE_SYSTEM_TEXT;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT_KEYBOARD, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    ES_PASSWORD, ES_READONLY, STATE_SYSTEM_FOCUSED, STATE_SYSTEM_PROTECTED, STATE_SYSTEM_READONLY,
};

use super::GLOBAL_VOICE_INPUT_TAG;
use super::access::integrity_allows;
use super::desktop::desktop_names_match;
use super::editable::legacy_state_allows;
use super::error::TargetError;
use super::input::unicode_inputs;
use super::native::{NativeFocus, is_native_edit, native_style_allows};

#[test]
fn native_password_and_readonly_are_never_editable() {
    assert_eq!(native_style_allows(0), Ok(()));
    assert_eq!(native_style_allows(ES_PASSWORD), Err(TargetError::Password));
    assert_eq!(native_style_allows(ES_READONLY), Err(TargetError::ReadOnly));
    assert_eq!(
        native_style_allows(ES_PASSWORD | ES_READONLY),
        Err(TargetError::Password)
    );
}

#[test]
fn native_edit_classes_do_not_treat_arbitrary_windows_as_inputs() {
    assert!(is_native_edit("Edit"));
    assert!(is_native_edit("RICHEDIT50W"));
    assert!(is_native_edit("RichEditD2DPT"));
    assert!(is_native_edit("WindowsForms10.EDIT.app.0.141b42a_r8_ad1"));
    assert!(!is_native_edit("Chrome_RenderWidgetHostHWND"));
    assert!(!is_native_edit("WeChatMainWndForPC"));
    assert!(!is_native_edit("Button"));
}

#[test]
fn legacy_text_requires_focus_and_rejects_password_readonly_disabled() {
    assert_eq!(
        legacy_state_allows(ROLE_SYSTEM_TEXT, STATE_SYSTEM_FOCUSED),
        Ok(())
    );
    for (state, error) in [
        (
            STATE_SYSTEM_FOCUSED | STATE_SYSTEM_PROTECTED,
            TargetError::Password,
        ),
        (
            STATE_SYSTEM_FOCUSED | STATE_SYSTEM_READONLY,
            TargetError::ReadOnly,
        ),
        (STATE_SYSTEM_FOCUSED | 1, TargetError::NoEditableTarget),
        (0, TargetError::NoEditableTarget),
    ] {
        assert_eq!(legacy_state_allows(ROLE_SYSTEM_TEXT, state), Err(error));
    }
    assert_eq!(
        legacy_state_allows(43, STATE_SYSTEM_FOCUSED),
        Err(TargetError::NoEditableTarget)
    );
}

#[test]
fn unicode_batch_preserves_chinese_punctuation_and_surrogate_pairs() {
    let text = "今天🙂，三点见。";
    let inputs = unicode_inputs(text).unwrap();
    let units: Vec<_> = text.encode_utf16().collect();
    assert_eq!(inputs.len(), units.len() * 2);
    for (pair, unit) in inputs.chunks_exact(2).zip(units) {
        assert_eq!(pair[0].r#type, INPUT_KEYBOARD);
        assert_eq!(pair[1].r#type, INPUT_KEYBOARD);
        let down = unsafe { pair[0].Anonymous.ki };
        let up = unsafe { pair[1].Anonymous.ki };
        assert_eq!(down.wScan, unit);
        assert_eq!(up.wScan, unit);
        assert_eq!(down.wVk.0, 0);
        assert_eq!(up.wVk.0, 0);
        assert_eq!(down.dwFlags, KEYEVENTF_UNICODE);
        assert_eq!(up.dwFlags, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP);
        assert_eq!(down.dwExtraInfo, GLOBAL_VOICE_INPUT_TAG);
        assert_eq!(up.dwExtraInfo, GLOBAL_VOICE_INPUT_TAG);
    }
}

#[test]
fn thousand_character_transcript_preserves_every_utf16_unit() {
    let text = format!(
        "开头🙂{}中段𠮷{}结尾🙂{}结束。",
        "今天早上我去公园散步".repeat(35),
        "明天下午我要整理工作".repeat(35),
        "周末我们一起准备晚饭".repeat(35),
    );
    assert!((1000..1200).contains(&text.chars().count()));
    let inputs = unicode_inputs(&text).unwrap();
    assert_eq!(inputs.len(), text.encode_utf16().count() * 2);
    for offset in [0, 1] {
        let restored: Vec<u16> = inputs
            .iter()
            .skip(offset)
            .step_by(2)
            .map(|input| unsafe { input.Anonymous.ki.wScan })
            .collect();
        assert_eq!(String::from_utf16(&restored).unwrap(), text);
    }
}

#[test]
fn control_characters_cannot_send_or_execute_the_transcript() {
    for text in [
        "你好\r\n",
        "echo hello\n",
        "一句\t话",
        "\0",
        "\u{1b}",
        "a\u{2028}b",
        "a\u{2029}b",
    ] {
        assert!(matches!(
            unicode_inputs(text),
            Err(TargetError::ControlCharacter)
        ));
    }
    assert!(unicode_inputs("").unwrap().is_empty());
    assert!(matches!(
        unicode_inputs(&"字".repeat(32_769)),
        Err(TargetError::TooLong)
    ));
}

#[test]
fn integrity_boundary_requires_uiaccess_and_still_excludes_system() {
    assert!(integrity_allows(0x2000, 0x2000, false));
    assert!(integrity_allows(0x2000, 0x1000, false));
    assert!(!integrity_allows(0x2000, 0x3000, false));
    assert!(integrity_allows(0x2000, 0x3000, true));
    assert!(!integrity_allows(0x2000, 0x4000, true));
}

#[test]
fn focus_identity_includes_both_windows_processes_and_thread() {
    let original = NativeFocus {
        foreground: HWND(1usize as _),
        focus: HWND(2usize as _),
        foreground_pid: 10,
        focus_pid: 11,
        thread: 12,
    };
    let changed = [
        NativeFocus {
            foreground: HWND(3usize as _),
            ..original
        },
        NativeFocus {
            focus: HWND(3usize as _),
            ..original
        },
        NativeFocus {
            foreground_pid: 13,
            ..original
        },
        NativeFocus {
            focus_pid: 13,
            ..original
        },
        NativeFocus {
            thread: 13,
            ..original
        },
    ];
    for current in changed {
        assert_ne!(original, current);
    }
}

#[test]
fn secure_or_unknown_desktop_is_rejected_even_if_window_focus_was_unchanged() {
    let ordinary: Vec<_> = "Default".encode_utf16().collect();
    let secure: Vec<_> = "Winlogon".encode_utf16().collect();
    assert!(desktop_names_match(&ordinary, &ordinary));
    assert!(!desktop_names_match(&secure, &ordinary));
    assert!(!desktop_names_match(&[], &[]));
    assert!(!desktop_names_match(&[0], &[0]));
}
