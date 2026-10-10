//! 全局按住触发回归：保护系统快捷键、AltGr 与取消后的轮次隔离。

use super::{
    activity::{ActivitySource, ignored_while_practicing, is_practice_escape},
    config::TriggerConfig,
    event::TriggerEvent,
    input::InputKey,
    machine::Machine,
    mode::VoiceMode,
};
use qingjian_platform::VoiceKey;
use std::time::{Duration, Instant};

fn config(key: VoiceKey) -> TriggerConfig {
    TriggerConfig {
        enabled: true,
        key,
        hold_ms: 180,
        practice_enabled: false,
        practice_shortcut: qingjian_platform::PracticeShortcut::DEFAULT,
    }
}

fn key(vk: u16, down: bool) -> InputKey {
    InputKey {
        vk,
        scan: if vk == 0xa4 || vk == 0xa5 { 0x38 } else { 0 },
        extended: vk == 0xa5,
        down,
    }
}

fn machine() -> Machine {
    Machine::new(config(VoiceKey::Alt), [false; 256])
}

#[test]
fn short_alt_replays_original_down_and_up_without_recording() {
    let now = Instant::now();
    let mut machine = machine();
    let press = machine.key(key(0xa5, true), 1, now);
    assert!(press.consume);
    assert_eq!(
        press.event,
        Some(TriggerEvent::Press {
            sequence: 1,
            revision: 1,
            mode: VoiceMode::Dictation,
        })
    );
    assert!(
        machine
            .tick(1, now + Duration::from_millis(90))
            .event
            .is_none()
    );
    let release = machine.key(key(0xa5, false), 2, now + Duration::from_millis(90));
    assert!(release.consume && release.replay_release);
    assert_eq!(release.replay_alt, Some(key(0xa5, true)));
    assert_eq!(
        release.event,
        Some(TriggerEvent::Release {
            sequence: 1,
            revision: 2
        })
    );
}

#[test]
fn long_alt_fires_once_and_never_replays_menu_activation() {
    let now = Instant::now();
    let mut machine = machine();
    machine.key(key(0xa5, true), 1, now);
    assert_eq!(
        machine.tick(1, now + Duration::from_millis(180)).event,
        Some(TriggerEvent::Held {
            sequence: 1,
            revision: 1
        })
    );
    assert!(
        machine
            .tick(1, now + Duration::from_millis(400))
            .event
            .is_none()
    );
    let release = machine.key(key(0xa5, false), 2, now + Duration::from_millis(600));
    assert!(release.consume);
    assert!(release.replay_alt.is_none());
    assert!(!release.replay_release);
}

#[test]
fn alt_tab_and_alt_f4_restore_alt_before_current_key() {
    for vk in [9, 0x73] {
        let now = Instant::now();
        let mut machine = machine();
        machine.key(key(0xa4, true), 1, now);
        let chord = machine.key(key(vk, true), 2, now);
        assert_eq!(
            chord.event,
            Some(TriggerEvent::Cancel {
                sequence: 1,
                revision: 2
            })
        );
        assert!(chord.consume && !chord.replay_release);
        assert_eq!(chord.replay_alt, Some(key(0xa4, true)));
        assert!(!machine.key(key(vk, false), 3, now).consume);
        assert!(!machine.key(key(0xa4, false), 4, now).consume);
    }
}

#[test]
fn alt_gr_control_prefix_never_starts_voice() {
    let now = Instant::now();
    for injected in [false, true] {
        let mut machine = machine();
        if injected {
            machine.injected_key(key(0xa2, true), 1);
        } else {
            machine.key(key(0xa2, true), 1, now);
        }
        assert!(!machine.key(key(0xa5, true), 2, now).consume);
        assert!(
            machine
                .tick(2, now + Duration::from_secs(1))
                .event
                .is_none()
        );
    }
}

#[test]
fn one_sided_configuration_preserves_other_alt() {
    let now = Instant::now();
    let mut machine = Machine::new(config(VoiceKey::RightAlt), [false; 256]);
    assert!(!machine.key(key(0xa4, true), 1, now).consume);
    machine.key(key(0xa4, false), 2, now);
    assert!(machine.key(key(0xa5, true), 3, now).consume);
    let other = machine.key(key(0xa4, false), 4, now);
    assert!(matches!(other.event, Some(TriggerEvent::Cancel { .. })));
    assert!(!matches!(other.event, Some(TriggerEvent::Release { .. })));
}

#[test]
fn mouse_button_or_wheel_cancels_but_preserves_alt_click() {
    let now = Instant::now();
    for button in [Some((1, true)), None] {
        let mut machine = machine();
        machine.key(key(0xa5, true), 1, now);
        let outcome = machine.mouse(button, 2);
        assert!(outcome.consume && outcome.replay_alt.is_some());
        assert!(matches!(outcome.event, Some(TriggerEvent::Cancel { .. })));
    }
}

#[test]
fn held_mouse_and_startup_held_alt_do_not_arm() {
    let now = Instant::now();
    let mut pressed = [false; 256];
    pressed[0xa5] = true;
    let mut machine = Machine::new(config(VoiceKey::Alt), pressed);
    assert!(!machine.key(key(0xa5, true), 1, now).consume);
    machine.key(key(0xa5, false), 2, now);
    machine.mouse(Some((1, true)), 3);
    assert!(!machine.key(key(0xa5, true), 4, now).consume);
}

#[test]
fn config_change_cancels_and_requires_fresh_press() {
    let now = Instant::now();
    let mut machine = machine();
    machine.key(key(0xa5, true), 1, now);
    assert!(machine.configure(config(VoiceKey::Alt), 2).event.is_none());
    assert!(matches!(
        machine.configure(config(VoiceKey::RightAlt), 3).event,
        Some(TriggerEvent::Cancel { sequence: 1, .. })
    ));
    assert!(!machine.key(key(0xa5, true), 4, now).consume);
    machine.key(key(0xa5, false), 5, now);
    assert_eq!(
        machine.key(key(0xa5, true), 6, now).event,
        Some(TriggerEvent::Press {
            sequence: 2,
            revision: 6,
            mode: VoiceMode::Dictation,
        })
    );
}

#[test]
fn injected_alt_never_arms_and_foreign_activity_cancels() {
    let now = Instant::now();
    let mut machine = machine();
    assert!(machine.injected_key(key(0xa5, true), 1).event.is_none());
    machine.injected_key(key(0xa5, false), 2);
    machine.key(key(0xa5, true), 3, now);
    assert!(matches!(
        machine.injected_key(key(0x41, true), 4).event,
        Some(TriggerEvent::Cancel { .. })
    ));
}

#[test]
fn hold_threshold_is_bounded() {
    let now = Instant::now();
    for (value, threshold) in [(0, 100), (10_000, 1000)] {
        let mut settings = config(VoiceKey::Alt);
        settings.hold_ms = value;
        let mut machine = Machine::new(settings, [false; 256]);
        machine.key(key(0xa5, true), 1, now);
        assert!(
            machine
                .tick(1, now + Duration::from_millis(threshold - 1))
                .event
                .is_none()
        );
        assert!(matches!(
            machine
                .tick(1, now + Duration::from_millis(threshold))
                .event,
            Some(TriggerEvent::Held { .. })
        ));
    }
}

#[test]
fn practice_keeps_open_across_pointer_and_focus_activity_but_escape_is_reserved() {
    for source in [
        ActivitySource::PhysicalKeyboard,
        ActivitySource::PhysicalMouse,
        ActivitySource::Foreground,
        ActivitySource::FocusNotification,
        ActivitySource::ForeignInjection,
    ] {
        assert!(ignored_while_practicing(source));
    }
    assert!(!ignored_while_practicing(ActivitySource::Configuration));
    assert!(is_practice_escape(true, true, 0x1b));
    assert!(!is_practice_escape(false, true, 0x1b));
    assert!(!is_practice_escape(true, false, 0x1b));
    assert!(!is_practice_escape(true, true, 0x41));
}

#[test]
fn practice_combination_requires_exact_modifiers_and_releases_once() {
    let now = Instant::now();
    let mut settings = config(VoiceKey::Alt);
    settings.practice_enabled = true;
    settings.practice_shortcut = "ctrl+alt+e".parse().unwrap();
    let mut machine = Machine::new(settings, [false; 256]);
    assert!(!machine.key(key(0x45, true), 1, now).consume);
    machine.key(key(0x45, false), 2, now);
    assert!(!machine.key(key(0xa2, true), 3, now).consume);
    assert!(!machine.key(key(0xa4, true), 4, now).consume);
    let press = machine.key(key(0x45, true), 5, now);
    assert!(matches!(press.event, Some(TriggerEvent::Press { .. })));
    assert!(press.consume);
    assert!(matches!(
        machine.tick(5, now + Duration::from_millis(200)).event,
        Some(TriggerEvent::Held { .. })
    ));
    let released = machine.key(key(0xa2, false), 6, now);
    assert!(matches!(released.event, Some(TriggerEvent::Release { .. })));
    assert!(!released.consume);
    let main_up = machine.key(key(0x45, false), 7, now);
    assert!(main_up.consume && main_up.event.is_none());
    assert!(main_up.replay_alt.is_none());
    assert!(!machine.changes_activity(key(0xa4, false)));
    machine.key(key(0xa4, false), 7, now);
    assert!(machine.changes_activity(key(0x41, true)));
}

#[test]
fn practice_observation_releases_chord_state_before_next_round() {
    let now = Instant::now();
    let mut settings = config(VoiceKey::Alt);
    settings.practice_enabled = true;
    settings.practice_shortcut = "ctrl+alt+e".parse().unwrap();
    let mut machine = Machine::new(settings, [false; 256]);
    machine.key(key(0xa2, true), 1, now);
    machine.key(key(0xa4, true), 2, now);
    machine.key(key(0x45, true), 3, now);
    machine.key(key(0xa2, false), 4, now);
    machine.observe_key(key(0x45, false));
    machine.observe_key(key(0xa4, false));
    machine.key(key(0xa2, true), 5, now);
    machine.key(key(0xa4, true), 6, now);
    assert!(matches!(
        machine.key(key(0x45, true), 7, now).event,
        Some(TriggerEvent::Press { .. })
    ));
}

#[test]
fn practice_shortcut_and_dictation_alt_are_independent() {
    let now = Instant::now();
    let mut settings = config(VoiceKey::RightAlt);
    settings.practice_enabled = true;
    settings.practice_shortcut = "f8".parse().unwrap();
    let mut machine = Machine::new(settings, [false; 256]);
    let dictate = machine.key(key(0xa5, true), 1, now);
    assert!(dictate.consume);
    assert!(matches!(
        dictate.event,
        Some(TriggerEvent::Press {
            mode: VoiceMode::Dictation,
            ..
        })
    ));
    machine.key(key(0xa5, false), 2, now);
    assert!(matches!(
        machine.key(key(0x77, true), 3, now).event,
        Some(TriggerEvent::Press {
            mode: VoiceMode::Practice,
            ..
        })
    ));
    machine.key(key(0x77, false), 4, now);
    settings.practice_enabled = false;
    machine.configure(settings, 5);
    assert!(matches!(
        machine.key(key(0xa5, true), 6, now).event,
        Some(TriggerEvent::Press {
            mode: VoiceMode::Dictation,
            ..
        })
    ));
    machine.key(key(0xa5, false), 7, now);
    assert!(!machine.key(key(0x77, true), 8, now).consume);
    assert!(machine.active.is_none());
}

#[test]
fn right_control_alt_accepts_both_orders_and_releases_exactly_once() {
    let now = Instant::now();
    for (first, last) in [(0xa3, 0xa5), (0xa5, 0xa3)] {
        for release_first in [first, last] {
            let mut settings = config(VoiceKey::RightAlt);
            settings.practice_enabled = true;
            settings.practice_shortcut = qingjian_platform::PracticeShortcut::RIGHT_CTRL_ALT;
            let mut machine = Machine::new(settings, [false; 256]);
            let partial = machine.key(key(first, true), 1, now);
            assert!(partial.event.is_none());
            assert_eq!(partial.consume, first == 0xa5);
            let start = machine.key(key(last, true), 2, now);
            assert!(matches!(
                start.event,
                Some(TriggerEvent::Press {
                    mode: VoiceMode::Practice,
                    ..
                })
            ));
            assert!(start.consume);
            assert!(start.replay_alt.is_none());
            assert!(machine.key(key(last, true), 2, now).event.is_none());
            assert!(matches!(
                machine.tick(2, now + Duration::from_millis(200)).event,
                Some(TriggerEvent::Held { .. })
            ));
            let end = machine.key(key(release_first, false), 3, now);
            assert!(matches!(end.event, Some(TriggerEvent::Release { .. })));
            assert!(end.replay_alt.is_none());
            assert_eq!(end.consume, first == 0xa5 || release_first == last);
            let remaining = if release_first == first { last } else { first };
            assert!(!machine.changes_activity(key(remaining, false)));
            let cleanup = machine.key(key(remaining, false), 3, now);
            assert!(cleanup.event.is_none());
            assert_eq!(cleanup.consume, first == 0xa5 || remaining == last);
            assert!(machine.active.is_none());
            machine.key(key(first, true), 4, now);
            assert!(matches!(
                machine.key(key(last, true), 5, now).event,
                Some(TriggerEvent::Press { .. })
            ));
        }
    }
}

#[test]
fn right_control_alt_rejects_left_modifiers_and_altgr() {
    let now = Instant::now();
    let mut settings = config(VoiceKey::RightAlt);
    settings.practice_enabled = true;
    settings.practice_shortcut = qingjian_platform::PracticeShortcut::RIGHT_CTRL_ALT;
    for sequence in [
        vec![0xa2, 0xa5],
        vec![0xa5, 0xa2],
        vec![0xa3, 0xa4],
        vec![0xa2, 0xa3, 0xa5],
        vec![0xa0, 0xa3, 0xa5],
    ] {
        let mut machine = Machine::new(settings, [false; 256]);
        for vk in sequence {
            assert!(machine.key(key(vk, true), 1, now).event.is_none());
        }
        assert!(machine.active.is_none());
    }
    let mut machine = Machine::new(settings, [false; 256]);
    machine.injected_key(key(0xa2, true), 1);
    assert!(machine.key(key(0xa5, true), 2, now).event.is_none());
    assert!(machine.active.is_none());
}

fn right_control_alt_machine() -> Machine {
    let mut settings = config(VoiceKey::RightAlt);
    settings.practice_enabled = true;
    settings.practice_shortcut = qingjian_platform::PracticeShortcut::RIGHT_CTRL_ALT;
    Machine::new(settings, [false; 256])
}

#[test]
fn right_alt_prefix_restores_single_alt_and_other_shortcuts() {
    let now = Instant::now();
    let mut machine = right_control_alt_machine();
    assert!(machine.key(key(0xa5, true), 1, now).consume);
    let repeat = machine.key(key(0xa5, true), 1, now);
    assert!(repeat.consume && repeat.replay_alt.is_none());
    let tap = machine.key(key(0xa5, false), 2, now);
    assert!(tap.consume && tap.replay_release);
    assert_eq!(tap.replay_alt, Some(key(0xa5, true)));
    assert!(tap.event.is_none());

    machine.key(key(0xa5, true), 3, now);
    let tab = machine.key(key(0x09, true), 4, now);
    assert!(tab.consume && !tab.replay_release);
    assert_eq!(tab.replay_alt, Some(key(0xa5, true)));
    assert!(tab.event.is_none());
    assert!(!machine.key(key(0x09, false), 5, now).consume);
    assert!(!machine.key(key(0xa5, false), 6, now).consume);
}

#[test]
fn right_alt_prefix_is_restored_on_cancel_mouse_and_configuration_change() {
    let now = Instant::now();
    for cause in 0..3 {
        let mut machine = right_control_alt_machine();
        machine.key(key(0xa5, true), 1, now);
        let restored = match cause {
            0 => machine.cancel(2),
            1 => machine.mouse(Some((1, true)), 2),
            _ => machine.configure(config(VoiceKey::RightAlt), 2),
        };
        assert_eq!(restored.replay_alt, Some(key(0xa5, true)));
        assert!(restored.event.is_none());
        assert!(!restored.replay_release);
        assert!(!machine.key(key(0xa5, false), 3, now).consume);
        assert!(machine.cancel(4).replay_alt.is_none());
    }
}

#[test]
fn right_alt_first_chord_keeps_both_swallowed_releases_paired_after_cancel() {
    let now = Instant::now();
    let mut machine = right_control_alt_machine();
    machine.key(key(0xa5, true), 1, now);
    machine.key(key(0xa3, true), 2, now);
    let cancel = machine.cancel(3);
    assert!(matches!(cancel.event, Some(TriggerEvent::Cancel { .. })));
    assert!(cancel.replay_alt.is_none());
    assert!(machine.key(key(0xa5, false), 4, now).consume);
    assert!(machine.key(key(0xa3, false), 5, now).consume);
    assert!(!machine.key(key(0xa3, false), 6, now).consume);
    assert!(!machine.key(key(0xa5, false), 7, now).consume);
}

#[test]
fn practice_observation_swallows_pending_alt_release_without_sticking() {
    let now = Instant::now();
    for (first_up, last_up) in [(0xa3, 0xa5), (0xa5, 0xa3)] {
        let mut machine = right_control_alt_machine();
        machine.key(key(0xa5, true), 1, now);
        machine.key(key(0xa3, true), 2, now);
        machine.key(key(first_up, false), 3, now);
        assert!(machine.observe_key(key(last_up, false)));
        assert!(!machine.observe_key(key(last_up, false)));
        assert!(!machine.key(key(0xa3, true), 4, now).consume);
        assert!(matches!(
            machine.key(key(0xa5, true), 5, now).event,
            Some(TriggerEvent::Press { .. })
        ));
    }
}

#[test]
fn left_alt_dictation_and_right_chord_practice_start_distinct_rounds() {
    let now = Instant::now();
    let mut settings = config(VoiceKey::LeftAlt);
    settings.practice_enabled = true;
    settings.practice_shortcut = qingjian_platform::PracticeShortcut::RIGHT_CTRL_ALT;
    let mut machine = Machine::new(settings, [false; 256]);

    let dictate = machine.key(key(0xa4, true), 1, now);
    assert_eq!(
        dictate.event,
        Some(TriggerEvent::Press {
            sequence: 1,
            revision: 1,
            mode: VoiceMode::Dictation,
        })
    );
    assert_eq!(machine.active.as_ref().unwrap().mode, VoiceMode::Dictation);
    machine.tick(1, now + Duration::from_millis(200));
    let released = machine.key(key(0xa4, false), 2, now);
    assert!(matches!(
        released.event,
        Some(TriggerEvent::Release { sequence: 1, .. })
    ));
    assert!(released.replay_alt.is_none());

    machine.key(key(0xa3, true), 3, now);
    let practice = machine.key(key(0xa5, true), 4, now);
    assert_eq!(
        practice.event,
        Some(TriggerEvent::Press {
            sequence: 2,
            revision: 4,
            mode: VoiceMode::Practice,
        })
    );
    assert_eq!(machine.active.as_ref().unwrap().mode, VoiceMode::Practice);
    machine.key(key(0xa5, false), 5, now);
    machine.observe_key(key(0xa3, false));

    let dictate_again = machine.key(key(0xa4, true), 6, now);
    assert!(matches!(
        dictate_again.event,
        Some(TriggerEvent::Press {
            sequence: 3,
            mode: VoiceMode::Dictation,
            ..
        })
    ));
}

#[test]
fn dual_entry_keeps_left_alt_system_shortcuts_and_short_taps() {
    let now = Instant::now();
    let mut settings = config(VoiceKey::LeftAlt);
    settings.practice_enabled = true;
    settings.practice_shortcut = qingjian_platform::PracticeShortcut::RIGHT_CTRL_ALT;
    for next in [0x09, 0x73] {
        let mut machine = Machine::new(settings, [false; 256]);
        machine.key(key(0xa4, true), 1, now);
        let chord = machine.key(key(next, true), 2, now);
        assert!(matches!(chord.event, Some(TriggerEvent::Cancel { .. })));
        assert_eq!(chord.replay_alt, Some(key(0xa4, true)));
        assert!(chord.consume && !chord.replay_release);
    }
    let mut machine = Machine::new(settings, [false; 256]);
    machine.key(key(0xa4, true), 1, now);
    let tap = machine.key(key(0xa4, false), 2, now);
    assert_eq!(tap.replay_alt, Some(key(0xa4, true)));
    assert!(tap.consume && tap.replay_release);
}

#[test]
fn legacy_both_alt_reserves_right_prefix_and_keeps_left_dictation() {
    let now = Instant::now();
    let mut settings = config(VoiceKey::Alt);
    settings.practice_enabled = true;
    settings.practice_shortcut = qingjian_platform::PracticeShortcut::RIGHT_CTRL_ALT;
    let mut machine = Machine::new(settings, [false; 256]);
    let left = machine.key(key(0xa4, true), 1, now);
    assert!(matches!(
        left.event,
        Some(TriggerEvent::Press {
            mode: VoiceMode::Dictation,
            ..
        })
    ));
    machine.key(key(0xa4, false), 2, now);
    assert!(machine.key(key(0xa5, true), 3, now).event.is_none());
    assert!(
        machine
            .tick(3, now + Duration::from_secs(1))
            .event
            .is_none()
    );
    let right = machine.key(key(0xa3, true), 4, now);
    assert!(matches!(
        right.event,
        Some(TriggerEvent::Press {
            mode: VoiceMode::Practice,
            ..
        })
    ));
}

#[test]
fn adding_practice_modifiers_cancels_dictation_without_changing_its_mode() {
    let now = Instant::now();
    let mut settings = config(VoiceKey::LeftAlt);
    settings.practice_enabled = true;
    settings.practice_shortcut = "ctrl+alt+e".parse().unwrap();
    let mut machine = Machine::new(settings, [false; 256]);
    machine.key(key(0xa4, true), 1, now);
    let chord = machine.key(key(0xa2, true), 2, now);
    assert!(matches!(
        chord.event,
        Some(TriggerEvent::Cancel { sequence: 1, .. })
    ));
    assert_eq!(chord.replay_alt, Some(key(0xa4, true)));
    let practice = machine.key(key(0x45, true), 3, now);
    assert!(matches!(
        practice.event,
        Some(TriggerEvent::Press {
            sequence: 2,
            mode: VoiceMode::Practice,
            ..
        })
    ));
}

#[test]
fn dual_entry_configuration_changes_cancel_active_mode_before_next_press() {
    let now = Instant::now();
    let mut settings = config(VoiceKey::LeftAlt);
    settings.practice_enabled = true;
    settings.practice_shortcut = qingjian_platform::PracticeShortcut::RIGHT_CTRL_ALT;
    let mut machine = Machine::new(settings, [false; 256]);
    machine.key(key(0xa4, true), 1, now);
    settings.practice_enabled = false;
    assert!(matches!(
        machine.configure(settings, 2).event,
        Some(TriggerEvent::Cancel { sequence: 1, .. })
    ));
    assert!(machine.key(key(0xa4, true), 3, now).event.is_none());
    machine.key(key(0xa4, false), 4, now);
    assert!(matches!(
        machine.key(key(0xa4, true), 5, now).event,
        Some(TriggerEvent::Press {
            sequence: 2,
            mode: VoiceMode::Dictation,
            ..
        })
    ));
}
