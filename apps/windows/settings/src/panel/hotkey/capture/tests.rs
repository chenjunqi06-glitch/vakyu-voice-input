//! 快捷键录入回归，不安装系统钩子或产生真实键盘输入。

use super::Keys;
use qingjian_platform::PracticeShortcut;

#[test]
fn right_ctrl_alt_records_in_either_order_after_both_keys_release() {
    for order in [[0xa3, 0xa5], [0xa5, 0xa3]] {
        for release in [order, [order[1], order[0]]] {
            let mut keys = Keys::new([false; 256]);
            for vk in order {
                assert!(keys.key(vk, true));
                assert!(keys.take_result().is_none());
            }
            assert!(keys.key(release[0], false));
            assert!(keys.take_result().is_none());
            assert!(keys.key(release[1], false));
            assert_eq!(
                keys.take_result(),
                Some(Ok(PracticeShortcut::RIGHT_CTRL_ALT))
            );
        }
    }
}

#[test]
fn adding_a_letter_keeps_the_full_combination() {
    let mut keys = Keys::new([false; 256]);
    for vk in [0xa3, 0xa5, 0x45] {
        keys.key(vk, true);
    }
    assert!(keys.key(0x45, true));
    assert!(keys.take_result().is_none());
    keys.key(0x45, false);
    assert!(keys.take_result().is_none());
    keys.key(0xa5, false);
    keys.key(0xa3, false);
    assert_eq!(keys.take_result(), Some(Ok("ctrl+alt+e".parse().unwrap())));
}

#[test]
fn unsupported_modifier_chord_reports_an_error_on_release() {
    for chord in [vec![0xa2, 0xa5], vec![0xa3], vec![0xa0]] {
        let mut keys = Keys::new([false; 256]);
        for &vk in &chord {
            keys.key(vk, true);
        }
        for vk in chord {
            keys.key(vk, false);
        }
        assert!(keys.take_result().unwrap().is_err());
    }
}

#[test]
fn single_alt_and_function_keys_still_record_and_letters_explain_rejection() {
    for (vk, expected) in [(0xa4, "left_alt"), (0xa5, "right_alt"), (0x77, "f8")] {
        let mut keys = Keys::new([false; 256]);
        keys.key(vk, true);
        assert!(keys.take_result().is_none());
        keys.key(vk, false);
        assert_eq!(keys.take_result(), Some(Ok(expected.parse().unwrap())));
    }
    let mut keys = Keys::new([false; 256]);
    keys.key(0x41, true);
    keys.key(0x41, false);
    assert!(keys.take_result().unwrap().is_err());
}

#[test]
fn preheld_modifier_release_is_not_swallowed() {
    let mut pressed = [false; 256];
    pressed[0xa3] = true;
    let mut keys = Keys::new(pressed);
    assert!(keys.key(0xa5, true));
    assert!(keys.key(0xa5, false));
    assert!(!keys.key(0xa3, false));
    assert_eq!(
        keys.take_result(),
        Some(Ok(PracticeShortcut::RIGHT_CTRL_ALT))
    );
}
