//! 不访问麦克风或网络的协议回归；直接喂当前/过期识别事件。

mod continuation;
mod lifecycle;
mod modes;

use super::{Controller, settings::validated_value};
use crate::protocol::{AsrEvent, Command, Effect, Event};
use qingjian_learning::{PracticeLevel, PracticeMatchConfig};
use qingjian_platform::Config;
use qingjian_practice::{PracticeAction, PracticeSession};
use serde_json::json;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

fn controller() -> Controller {
    let mut controller = Controller::new(
        Config::default(),
        PathBuf::from("unused-config.toml"),
        PathBuf::from("unused-data"),
    );
    // 测试不允许写经验文件。
    controller.game = crate::game::PracticeGame::load(std::path::Path::new("unused-data"), true);
    controller.backend(AsrEvent::Ready);
    controller.drain();
    controller
}

fn reading(english: bool) -> Controller {
    let mut c = controller();
    c.config.shortcut.voice_practice_learn_english = english;
    c.config.shortcut.voice_english_practice = true;
    c.active = true;
    c.round_mode = crate::protocol::Mode::Practice;
    let mut session = PracticeSession::new_for_language(
        "Original text".to_owned(),
        PracticeLevel::Challenge,
        String::new(),
        PracticeMatchConfig::default(),
        english,
    );
    assert!(session.translated(if english {
        "I want rice."
    } else {
        "我想吃饭。"
    }));
    c.practice = Some(session);
    c.start_practice_capture();
    c.backend(AsrEvent::Started {
        generation: c.generation,
    });
    c.drain();
    c
}

#[test]
fn stale_audio_and_repeat_presses_never_start_or_commit_another_round() {
    let mut c = controller();
    c.command(Command::Press { mode: None });
    let generation = c.generation;
    c.drain();
    c.command(Command::Press { mode: None });
    assert!(c.drain().is_empty());
    c.command(Command::Cancel);
    c.drain();
    c.backend(AsrEvent::Final {
        generation,
        text: "stale text".to_owned(),
    });
    assert!(c.drain().is_empty());
    c.command(Command::Audio {
        generation,
        sample_rate: 16000,
        pcm: "AAAAAA==".to_owned(),
    });
    assert!(c.drain().is_empty());
}

#[test]
fn dictation_only_commits_after_release_and_only_once() {
    let mut c = controller();
    c.command(Command::Press { mode: None });
    let generation = c.generation;
    c.backend(AsrEvent::Started { generation });
    c.backend(AsrEvent::Interim {
        generation,
        text: "hello".to_owned(),
    });
    assert!(
        !c.drain()
            .iter()
            .any(|e| matches!(e, Effect::Native(Event::Commit { .. })))
    );
    c.command(Command::Release);
    c.backend(AsrEvent::Final {
        generation,
        text: "hello".to_owned(),
    });
    let events = c.drain();
    let id = events
        .iter()
        .find_map(|e| {
            if let Effect::Native(Event::Commit { id, .. }) = e {
                Some(*id)
            } else {
                None
            }
        })
        .unwrap();
    c.backend(AsrEvent::Final {
        generation,
        text: "hello".to_owned(),
    });
    assert!(
        !c.drain()
            .iter()
            .any(|e| matches!(e, Effect::Native(Event::Commit { .. })))
    );
    c.command(Command::CommitResult {
        id,
        ok: true,
        message: None,
    });
    assert!(!c.active);
    assert!(
        c.drain()
            .iter()
            .any(|e| matches!(e, Effect::Native(Event::Hide)))
    );
}

#[test]
fn spontaneous_final_does_not_cut_a_long_held_recording_and_insert() {
    let mut c = controller();
    c.command(Command::Press { mode: None });
    c.backend(AsrEvent::Started {
        generation: c.generation,
    });
    c.tick(Instant::now() + Duration::from_secs(3600));
    assert!(c.capturing);
    c.backend(AsrEvent::Final {
        generation: c.generation,
        text: "unexpected".to_owned(),
    });
    assert!(
        !c.drain()
            .iter()
            .any(|e| matches!(e, Effect::Native(Event::Commit { .. })))
    );
}

#[test]
fn bilingual_gate_requires_final_result_then_native_success_and_retry_keeps_text() {
    for english in [true, false] {
        let mut c = reading(english);
        let transcript = if english {
            "I want rice"
        } else {
            "我想吃饭"
        };
        c.backend(AsrEvent::Interim {
            generation: c.generation,
            text: transcript.to_owned(),
        });
        assert!(c.pending_commit.is_none());
        c.tick(Instant::now() + Duration::from_secs(1));
        c.backend(AsrEvent::Final {
            generation: c.generation,
            text: transcript.to_owned(),
        });
        let id = c.pending_commit.as_ref().unwrap().0;
        assert!(c.practice.is_some());
        c.command(Command::CommitResult {
            id,
            ok: false,
            message: Some("field changed".to_owned()),
        });
        assert!(c.practice.is_some());
        c.command(Command::Action {
            action: PracticeAction::Retry,
            index: None,
        });
        let (retry_id, text, _) = c.pending_commit.as_ref().unwrap();
        assert_eq!(*retry_id, id);
        assert_eq!(text, "Original text");
        let retry_id = *retry_id;
        c.command(Command::CommitResult {
            id: retry_id,
            ok: true,
            message: None,
        });
        assert!(c.practice.is_none());
    }
}

#[test]
fn lookup_keeps_generation_progress_and_demo_audio_cannot_pass_gate() {
    let mut c = reading(true);
    c.backend(AsrEvent::Interim {
        generation: c.generation,
        text: "I want".to_owned(),
    });
    let generation = c.generation;
    let matched = c.practice.as_ref().unwrap().view().matched_words;
    c.command(Command::Action {
        action: PracticeAction::Lookup,
        index: Some(2),
    });
    assert_eq!(c.generation, generation);
    assert_eq!(c.practice.as_ref().unwrap().view().matched_words, matched);
    c.command(Command::Action {
        action: PracticeAction::SpeakWord,
        index: None,
    });
    assert!(!c.capturing);
    c.backend(AsrEvent::Interim {
        generation,
        text: "I want rice".to_owned(),
    });
    assert_eq!(c.practice.as_ref().unwrap().view().matched_words, matched);
    assert!(c.pending_commit.is_none());
}

#[test]
fn invalid_or_secret_settings_cannot_be_saved() {
    assert!(validated_value("api_key", &json!("secret")).is_err());
    assert!(validated_value("voice_practice_similarity", &json!(101)).is_err());
    assert!(validated_value("voice_practice_shortcut", &json!("a")).is_err());
    assert!(validated_value("voice_practice_shortcut", &json!("right_ctrl+right_alt")).is_ok());
    assert!(validated_value("voice_practice_learn_english", &json!(false)).is_ok());
}
