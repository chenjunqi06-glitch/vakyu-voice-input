//! 读完后填入原话还是译文：只有完整跟读才交付，开关只决定交付哪一份。

use super::{reading, strict_session};
use crate::Phase;
use std::time::Instant;

#[test]
fn default_round_still_inserts_the_original() {
    let mut session = reading();
    assert!(!session.sends_translation());
    session.interim("I want to eat rice today", Instant::now());
    session.checking();
    assert!(session.final_result("I want to eat rice today."));
    assert_eq!(session.completed_output(), Some("我今天想吃米饭。"));
}

#[test]
fn send_translation_inserts_the_english_that_was_read() {
    let mut session = strict_session("我今天想吃米饭。").with_send_translation(true);
    assert!(session.sends_translation());
    assert!(session.translated("I want to eat rice today."));
    session.started();
    assert!(session.view().status.contains("填入英文"));
    assert_eq!(session.completed_output(), None, "没读完不能交付译文");
    session.interim("I want to eat rice today", Instant::now());
    session.checking();
    assert!(session.final_result("I want to eat rice today."));
    assert_eq!(session.phase, Phase::Complete);
    assert_eq!(
        session.completed_output(),
        Some("I want to eat rice today.")
    );
    assert_eq!(session.completed_original(), Some("我今天想吃米饭。"));
}

#[test]
fn failed_translation_never_releases_anything() {
    let mut session = strict_session("我今天想吃米饭。").with_send_translation(true);
    session.fail("翻译失败".to_owned());
    assert_eq!(session.completed_output(), None);
}
