//! 普通口述与跟读快捷键有独立入口，模式在原话采集前锁定。

use super::controller;
use crate::protocol::{AsrEvent, Command, Effect, Event, Mode};

#[test]
fn explicit_dictation_never_translates_even_when_practice_and_reverse_direction_are_enabled() {
    let mut c = controller();
    c.config.shortcut.voice_english_practice = true;
    c.config.shortcut.voice_practice_learn_english = false;
    c.command(Command::Press {
        mode: Some(Mode::Dictation),
    });
    assert_eq!(c.round_mode, Mode::Dictation);
    assert_eq!(c.source_language(), "zh");
    c.backend(AsrEvent::Started {
        generation: c.generation,
    });
    c.command(Command::Release);
    c.backend(AsrEvent::Final {
        generation: c.generation,
        text: "本机口述".to_owned(),
    });
    assert!(c.practice.is_none());
    assert!(c.translator.is_none());
    assert_eq!(c.pending_commit.as_ref().unwrap().1, "本机口述");
}

#[test]
fn practice_disabled_never_falls_back_to_ordinary_dictation() {
    let mut c = controller();
    c.command(Command::Press {
        mode: Some(Mode::Practice),
    });
    assert!(!c.active);
    assert!(!c.capturing);
    assert!(c.pending_commit.is_none());
    let events = c.drain();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, Effect::Native(Event::Error { .. })))
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Effect::Native(Event::Capture { active: true, .. })))
    );
}

#[test]
fn duplicate_entry_and_practice_changes_cannot_change_ordinary_round() {
    let mut c = controller();
    c.command(Command::Press {
        mode: Some(Mode::Dictation),
    });
    let generation = c.generation;
    let mut config = c.config.clone();
    config.shortcut.voice_english_practice = true;
    config.shortcut.voice_practice_learn_english = false;
    c.apply_config(config);
    c.command(Command::Press {
        mode: Some(Mode::Practice),
    });
    assert_eq!(c.round_mode, Mode::Dictation);
    assert_eq!(c.generation, generation);
    assert!(c.capturing);
    assert!(!c.requires_new_round);
    assert_eq!(c.source_language(), "zh");
}

#[test]
fn explicit_reverse_practice_uses_english_source_and_cancel_drops_mode_ownership() {
    let mut c = controller();
    c.config.shortcut.voice_english_practice = true;
    c.config.shortcut.voice_practice_learn_english = false;
    c.command(Command::Press {
        mode: Some(Mode::Practice),
    });
    assert_eq!(c.source_language(), "en");
    let generation = c.generation;
    c.command(Command::Press {
        mode: Some(Mode::Dictation),
    });
    assert_eq!(c.generation, generation);
    assert_eq!(c.round_mode, Mode::Practice);
    c.command(Command::Cancel);
    c.command(Command::Press {
        mode: Some(Mode::Dictation),
    });
    assert_eq!(c.source_language(), "zh");
    assert_eq!(c.round_mode, Mode::Dictation);
}

#[test]
fn legacy_press_defaults_follow_existing_enablement_but_new_json_is_explicit() {
    let legacy: Command = serde_json::from_str(r#"{"command":"press"}"#).unwrap();
    assert!(matches!(legacy, Command::Press { mode: None }));
    let explicit: Command =
        serde_json::from_str(r#"{"command":"press","mode":"dictation"}"#).unwrap();
    assert!(matches!(
        explicit,
        Command::Press {
            mode: Some(Mode::Dictation)
        }
    ));
    let mut c = controller();
    c.config.shortcut.voice_english_practice = true;
    c.command(legacy);
    assert_eq!(c.round_mode, Mode::Practice);
}
