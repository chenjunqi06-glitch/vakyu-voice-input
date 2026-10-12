//! 本轮译句提示的管道隔离；预热、松键和取消均不能补开麦或污染普通口述。

use std::cell::RefCell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;

use qingjian_core::Language;
use qingjian_platform::protocol::VoiceUpdate;

use crate::voice::neural::closing::CloseWatchdog;
use crate::voice::neural::command::HelperCommand;
use crate::voice::neural::event::HelperEvent;
use crate::voice::neural::session::Session;
use crate::voice::neural::worker::reconcile_with_practice;

const EXPECTED: &str = "My name is Chen Junqi.";

#[test]
fn both_recognition_directions_reach_helper_with_scoped_language() {
    for (language, text) in [
        (Language::Chinese, "我今天去吃饭"),
        (Language::English, EXPECTED),
    ] {
        for practice in [false, true] {
            let mut closing = CloseWatchdog::default();
            let desired = AtomicU64::new(15);
            let mut active = Some(if practice {
                Session::practice_in_language(7, text.to_owned(), language)
            } else {
                Session::in_language(7, language)
            });
            let (updates, _results) = mpsc::channel();
            let sent = RefCell::new(Vec::new());
            let send =
                |command, generation, practice_text: Option<&str>, language: Option<Language>| {
                    sent.borrow_mut().push(
                        serde_json::to_value(HelperCommand {
                            command,
                            generation,
                            practice_text: practice_text.map(str::to_owned),
                            language: language.map(Language::code),
                        })
                        .unwrap(),
                    );
                    Ok(())
                };
            reconcile_with_practice(&send, &mut active, &updates, &desired, true, &mut closing)
                .unwrap();
            desired.store(14, Ordering::Release);
            reconcile_with_practice(&send, &mut active, &updates, &desired, true, &mut closing)
                .unwrap();
            let commands = sent.borrow();
            assert_eq!(commands[0]["language"], language.code());
            assert_eq!(
                commands[0]["practice_text"].as_str(),
                practice.then_some(text)
            );
            assert_eq!(commands[1]["command"], "stop");
            assert!(commands[1].get("language").is_none());
            assert!(commands[1].get("practice_text").is_none());
        }
    }
}

#[test]
fn practice_warmup_sends_hint_only_once_and_stop_has_no_hint() {
    let mut closing = CloseWatchdog::default();
    let desired = AtomicU64::new(15);
    let mut active = Some(Session::practice(7, EXPECTED.to_owned()));
    let (updates, results) = mpsc::channel();
    let sent = RefCell::new(Vec::new());
    let send = |command, generation, practice_text: Option<&str>, _language| {
        sent.borrow_mut()
            .push((command, generation, practice_text.map(str::to_owned)));
        Ok(())
    };
    reconcile_with_practice(&send, &mut active, &updates, &desired, false, &mut closing).unwrap();
    assert!(sent.borrow().is_empty());
    reconcile_with_practice(&send, &mut active, &updates, &desired, true, &mut closing).unwrap();
    reconcile_with_practice(&send, &mut active, &updates, &desired, true, &mut closing).unwrap();
    desired.store(14, Ordering::Release);
    reconcile_with_practice(&send, &mut active, &updates, &desired, true, &mut closing).unwrap();
    reconcile_with_practice(&send, &mut active, &updates, &desired, true, &mut closing).unwrap();
    assert_eq!(
        *sent.borrow(),
        vec![("start", 7, Some(EXPECTED.to_owned())), ("stop", 7, None)]
    );
    assert!(results.try_recv().is_err());
}

#[test]
fn stopped_or_cancelled_cold_practice_never_dispatches_hint_or_audio() {
    for intent in [14, 16] {
        let mut closing = CloseWatchdog::default();
        let desired = AtomicU64::new(15);
        let mut active = Some(Session::practice(7, EXPECTED.to_owned()));
        let (updates, results) = mpsc::channel();
        let sent = RefCell::new(Vec::new());
        let send = |command, generation, practice_text: Option<&str>, _language| {
            sent.borrow_mut()
                .push((command, generation, practice_text.map(str::to_owned)));
            Ok(())
        };
        reconcile_with_practice(&send, &mut active, &updates, &desired, false, &mut closing)
            .unwrap();
        desired.store(intent, Ordering::Release);
        reconcile_with_practice(&send, &mut active, &updates, &desired, true, &mut closing)
            .unwrap();
        assert!(sent.borrow().is_empty());
        assert!(active.is_none());
        if intent == 14 {
            assert_eq!(
                results.recv().unwrap(),
                (
                    7,
                    VoiceUpdate::Final {
                        text: String::new()
                    }
                )
            );
        } else {
            assert!(results.try_recv().is_err());
        }
        desired.store(17, Ordering::Release);
        active = Some(Session::new(8));
        reconcile_with_practice(&send, &mut active, &updates, &desired, true, &mut closing)
            .unwrap();
        assert_eq!(*sent.borrow(), vec![("start", 8, None)]);
    }
}

#[test]
fn active_practice_cancel_drops_hint_before_next_ordinary_start() {
    let mut closing = CloseWatchdog::default();
    let desired = AtomicU64::new(15);
    let mut active = Some(Session::practice(7, EXPECTED.to_owned()));
    let (updates, results) = mpsc::channel();
    let sent = RefCell::new(Vec::new());
    let send = |command, generation, practice_text: Option<&str>, _language| {
        sent.borrow_mut()
            .push((command, generation, practice_text.map(str::to_owned)));
        Ok(())
    };
    reconcile_with_practice(&send, &mut active, &updates, &desired, true, &mut closing).unwrap();
    desired.store(17, Ordering::Release);
    reconcile_with_practice(&send, &mut active, &updates, &desired, true, &mut closing).unwrap();
    assert!(active.is_none());
    assert!(results.try_recv().is_err());
    assert!(closing.acknowledge(&HelperEvent::Cancelled { generation: 7 }));
    active = Some(Session::new(8));
    reconcile_with_practice(&send, &mut active, &updates, &desired, true, &mut closing).unwrap();
    assert_eq!(
        *sent.borrow(),
        vec![
            ("start", 7, Some(EXPECTED.to_owned())),
            ("cancel", 7, None),
            ("start", 8, None)
        ]
    );
}

#[test]
fn ordinary_protocol_omits_practice_text_and_practice_preserves_it() {
    let ordinary = serde_json::to_value(HelperCommand {
        command: "start",
        generation: 7,
        language: Some("zh"),
        practice_text: None,
    })
    .unwrap();
    assert!(ordinary.get("practice_text").is_none());
    let practice = serde_json::to_value(HelperCommand {
        command: "start",
        generation: 8,
        language: Some("en"),
        practice_text: Some(EXPECTED.to_owned()),
    })
    .unwrap();
    assert_eq!(practice["practice_text"], EXPECTED);
    assert_eq!(practice["generation"], 8);
}

#[test]
fn audio_level_is_separate_from_transcript_and_is_scoped_to_generation() {
    let event: HelperEvent =
        serde_json::from_str(r#"{"event":"level","generation":9,"level":74}"#).unwrap();
    assert_eq!(event.generation(), Some(9));
    assert!(matches!(event, HelperEvent::Level { level: 74, .. }));
    assert!(serde_json::from_str::<HelperEvent>(r#"{"event":"level","level":74}"#).is_err());
    assert!(
        serde_json::from_str::<HelperEvent>(r#"{"event":"level","generation":9,"level":-1}"#)
            .is_err()
    );
}
