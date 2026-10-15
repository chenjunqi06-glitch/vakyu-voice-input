//! 配置、提交确认与用户可见会话的生命周期回归。

use super::{controller, reading};
use crate::protocol::{AsrEvent, Command, Effect, Event};
use qingjian_practice::PracticeAction;
use std::time::{Duration, Instant};

#[test]
fn unrelated_settings_keep_microphone_progress_and_window() {
    let mut c = reading(true);
    c.backend(AsrEvent::Interim {
        generation: c.generation,
        text: "I want".to_owned(),
    });
    let generation = c.generation;
    let matched = c.practice.as_ref().unwrap().view().matched_words;
    let mut config = c.config.clone();
    config.shortcut.voice_practice_similarity = 90;
    config.shortcut.voice_hold_ms = 200;
    config.shortcut.voice_practice_demo = false;
    c.drain();
    c.apply_config(config);
    assert!(c.capturing);
    assert_eq!(generation, c.generation);
    assert_eq!(matched, c.practice.as_ref().unwrap().view().matched_words);
    assert!(
        !c.drain()
            .iter()
            .any(|event| matches!(event, Effect::Native(Event::Hide | Event::CancelSpeech)))
    );
}

#[test]
fn changing_direction_stops_audio_but_keeps_original_until_escape() {
    let mut c = reading(true);
    let mut config = c.config.clone();
    config.shortcut.voice_practice_learn_english = false;
    c.apply_config(config);
    assert!(!c.capturing);
    assert!(c.requires_new_round);
    assert_eq!(
        c.practice.as_ref().unwrap().view().original,
        "Original text"
    );
    assert!(
        !c.drain()
            .iter()
            .any(|event| matches!(event, Effect::Native(Event::Hide)))
    );
    c.command(Command::Action {
        action: PracticeAction::Retry,
        index: None,
    });
    assert!(!c.capturing);
    assert!(c.pending_commit.is_none());
    c.command(Command::Cancel);
    assert!(c.practice.is_none());
}

#[test]
fn ordinary_dictation_retry_reuses_id_after_failure_or_lost_ack() {
    let mut c = controller();
    c.command(Command::Press { mode: None });
    c.backend(AsrEvent::Started {
        generation: c.generation,
    });
    c.command(Command::Release);
    c.backend(AsrEvent::Final {
        generation: c.generation,
        text: "Keep this original".to_owned(),
    });
    let id = c.pending_commit.as_ref().unwrap().0;
    c.command(Command::CommitResult {
        id,
        ok: false,
        message: None,
    });
    assert!(c.active);
    c.command(Command::Action {
        action: PracticeAction::Retry,
        index: None,
    });
    assert_eq!(c.pending_commit.as_ref().unwrap().0, id);
    c.tick(Instant::now() + Duration::from_secs(16));
    assert!(c.pending_commit.is_none());
    c.command(Command::Action {
        action: PracticeAction::Retry,
        index: None,
    });
    let pending = c.pending_commit.as_ref().unwrap();
    assert_eq!(pending.0, id);
    assert_eq!(pending.1, "Keep this original");
    c.command(Command::CommitResult {
        id,
        ok: true,
        message: None,
    });
    assert!(!c.active);
    assert!(c.delivery.is_none());
}

#[test]
fn idle_service_sleeps_but_a_press_immediately_requests_fast_polling() {
    let mut c = controller();
    assert_eq!(c.poll_interval(), Duration::from_secs(1));
    c.command(Command::Press { mode: None });
    assert_eq!(c.poll_interval(), Duration::from_millis(20));
}
