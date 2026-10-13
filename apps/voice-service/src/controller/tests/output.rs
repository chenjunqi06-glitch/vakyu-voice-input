//! 读完发出译文与划词翻译：只改变交付内容或旁路回复，不改变跟读门禁。

use super::{controller, reading};
use crate::protocol::{AsrEvent, Command, Effect, Event};
use std::time::{Duration, Instant};

#[test]
fn send_translation_commits_the_text_that_was_read() {
    let mut c = reading(true);
    let session = c.practice.take().unwrap().with_send_translation(true);
    c.practice = Some(session);
    c.backend(AsrEvent::Interim {
        generation: c.generation,
        text: "I want rice".to_owned(),
    });
    c.tick(Instant::now() + Duration::from_secs(1));
    c.backend(AsrEvent::Final {
        generation: c.generation,
        text: "I want rice".to_owned(),
    });
    assert_eq!(c.pending_commit.as_ref().unwrap().1, "I want rice.");
}

#[test]
fn default_round_still_commits_the_original() {
    let mut c = reading(true);
    c.backend(AsrEvent::Interim {
        generation: c.generation,
        text: "I want rice".to_owned(),
    });
    c.tick(Instant::now() + Duration::from_secs(1));
    c.backend(AsrEvent::Final {
        generation: c.generation,
        text: "I want rice".to_owned(),
    });
    assert_eq!(c.pending_commit.as_ref().unwrap().1, "Original text");
}

#[test]
fn selection_translation_reports_setup_errors_without_touching_practice() {
    let mut c = controller();
    c.config.predict.base_url = "not a url".to_owned();
    c.command(Command::Translate {
        id: 9,
        text: "今天下午开会".to_owned(),
    });
    let effects = c.drain();
    assert!(effects.iter().any(|effect| matches!(
        effect,
        Effect::Native(Event::Translation { id: 9, ok: false, target, .. }) if target == "en"
    )));
    assert!(c.reading.is_none());
    assert!(!c.active);
    assert!(c.practice.is_none());
}

#[test]
fn send_translation_is_an_editable_setting() {
    assert!(
        super::validated_value("voice_practice_send_translation", &serde_json::json!(true)).is_ok()
    );
    assert!(
        super::validated_value("voice_practice_send_translation", &serde_json::json!(1)).is_err()
    );
}
