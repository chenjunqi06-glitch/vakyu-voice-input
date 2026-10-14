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
fn send_translation_is_an_editable_setting() {
    assert!(
        super::validated_value("voice_practice_send_translation", &serde_json::json!(true)).is_ok()
    );
    assert!(
        super::validated_value("voice_practice_send_translation", &serde_json::json!(1)).is_err()
    );
}

fn local_round(english: bool) -> crate::controller::Controller {
    let mut c = controller();
    c.config.shortcut.voice_english_practice = true;
    c.config.shortcut.voice_practice_learn_english = english;
    c.config.shortcut.voice_practice_demo = false;
    c.active = true;
    c.round_mode = crate::protocol::Mode::Practice;
    c.begin_practice("我今天想吃米饭。".to_owned());
    c
}

#[test]
fn practice_asks_the_host_and_accepts_its_translation() {
    let mut c = local_round(true);
    let effects = c.drain();
    let round = c.round;
    assert!(effects.iter().any(|effect| matches!(
        effect,
        Effect::Native(Event::TranslateRequest { id, target, .. }) if *id == round && target == "en"
    )));
    c.command(Command::Translated {
        id: round,
        ok: true,
        text: "  I want to eat\nrice today. ".to_owned(),
    });
    let session = c.practice.as_ref().unwrap();
    assert_eq!(session.phase, qingjian_practice::Phase::Preparing);
    assert_eq!(session.recognition_hint(), "I want to eat rice today.");
}

#[test]
fn stale_reports_are_ignored_and_failures_keep_the_original() {
    let mut c = local_round(true);
    let round = c.round;
    c.command(Command::Translated {
        id: round + 1,
        ok: true,
        text: "stale".to_owned(),
    });
    assert_eq!(
        c.practice.as_ref().unwrap().phase,
        qingjian_practice::Phase::Translating
    );
    c.command(Command::Translated {
        id: round,
        ok: false,
        text: "请先下载翻译语言".to_owned(),
    });
    let session = c.practice.as_ref().unwrap();
    assert_eq!(session.phase, qingjian_practice::Phase::Failed);
    assert_eq!(session.view().feedback, "请先下载翻译语言");
    assert_eq!(session.original(), "我今天想吃米饭。");
}
