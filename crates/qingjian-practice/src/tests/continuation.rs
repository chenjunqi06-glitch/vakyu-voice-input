//! 已通过词的保留、累计识别去重、停顿续读与用户主动整句重读。

use std::time::{Duration, Instant};

use qingjian_learning::{PracticeLevel, PracticeMatchConfig};

use super::{reading, strict_session};
use crate::{Phase, PracticeAction, PracticeSession, lookup};

fn repeated_session(english: bool) -> PracticeSession {
    let mut session = PracticeSession::new_for_language(
        "原文".to_owned(),
        PracticeLevel::Challenge,
        String::new(),
        PracticeMatchConfig::default(),
        english,
    );
    assert!(session.translated(if english {
        "go go go home"
    } else {
        "看看看看书"
    }));
    session.started();
    session
}

#[test]
fn repeated_snapshots_and_normalization_do_not_reuse_a_spoken_word() {
    let now = Instant::now();
    let mut session = repeated_session(true);
    for text in ["go", "GO!", " go  ", "go", "", "go"] {
        session.interim(text, now);
        assert_eq!(session.view().matched_words, 1, "{text}");
    }
    session.interim("go go", now);
    assert_eq!(session.view().matched_words, 2);
    for text in ["go", "", "go go", "GO, GO!"] {
        session.interim(text, now);
        assert_eq!(session.view().matched_words, 2, "{text}");
    }
    session.checking();
    assert!(!session.final_result("go go"));
    assert_eq!(session.view().matched_words, 2);
    assert!(session.take_original().is_none());
    session.started();
    session.interim("go home", now);
    session.checking();
    assert!(session.final_result("go home"));
}

#[test]
fn chinese_repeated_characters_need_distinct_recognition_evidence() {
    let now = Instant::now();
    let mut session = repeated_session(false);
    for text in ["看", "看。", "", "看"] {
        session.interim(text, now);
        assert_eq!(session.view().matched_words, 1, "{text}");
    }
    session.interim("看看", now);
    assert_eq!(session.view().matched_words, 2);
    session.interim("看", now);
    session.interim("看看", now);
    assert_eq!(session.view().matched_words, 2);
    session.interim("看看看看书", now);
    assert_eq!(session.view().matched_words, 5);
    session.checking();
    assert!(session.final_result(""));
}

#[test]
fn corrected_old_words_do_not_block_new_current_word_evidence() {
    let now = Instant::now();
    let mut session = reading();
    session.interim("I want to eat", now);
    assert_eq!(session.view().matched_words, 4);
    session.interim("I want to sleep", now);
    assert_eq!(session.view().matched_words, 4);
    session.interim("I want to sleep rice", now);
    assert_eq!(session.view().matched_words, 5);
    assert!(session.take_original().is_none());
    session.interim("I want to sleep rice today", now);
    session.checking();
    assert!(session.final_result("I want to sleep rice today"));
}

#[test]
fn a_wrong_word_can_be_retried_alone_in_the_same_recording() {
    let now = Instant::now();
    let mut session = reading();
    session.interim("I want to eat fish", now);
    assert_eq!(session.view().matched_words, 4);
    session.interim("I want to eat fish rice", now);
    assert_eq!(session.view().matched_words, 5);
    session.interim("I want to eat fish rice today", now);
    session.checking();
    assert!(session.final_result("I want to eat fish rice today"));
}

#[test]
fn empty_final_and_microphone_retry_preserve_the_gate_and_progress() {
    let now = Instant::now();
    let mut session = reading();
    session.interim("I want to eat", now);
    session.checking();
    assert!(!session.final_result(""));
    assert_eq!(session.view().matched_words, 4);
    assert!(session.take_original().is_none());
    assert!(session.retry_current());
    session.started();
    session.interim("rice", now + Duration::from_secs(60));
    assert_eq!(session.view().matched_words, 5);
    assert!(session.take_original().is_none());
    session.checking();
    assert!(!session.final_result("rice"));
    session.started();
    session.interim("today", now + Duration::from_secs(120));
    session.checking();
    assert!(session.final_result(""));
    assert!(session.take_original().is_some());
}

#[test]
fn restart_explicitly_clears_progress_feedback_and_card_but_reuses_translation() {
    let now = Instant::now();
    let mut session = reading();
    session.interim("I want to eat", now);
    session.show_word_card(lookup::word_card_for_language("rice", true));
    session.fail("麦克风暂不可用".to_owned());
    assert!(session.restart_sentence());
    assert_eq!(session.phase, Phase::Preparing);
    let view = session.view();
    assert_eq!(view.matched_words, 0);
    assert!(view.recognized.is_empty());
    assert!(view.feedback.is_empty());
    assert!(view.word_card.is_none());
    assert_eq!(session.recognition_hint(), "I want to eat rice today.");
    assert_eq!(view.original, "我今天想吃米饭。");
    assert!(!session.should_check(now + Duration::from_secs(30)));
    session.started();
    session.interim("rice today", now);
    assert_eq!(session.view().matched_words, 0);
    session.checking();
    assert!(!session.final_result("rice today"));
    session.started();
    session.interim("I want to eat rice today", now);
    session.checking();
    assert!(session.final_result("I want to eat rice today"));
    assert!(
        !session.restart_sentence(),
        "完成并交给填入流程后不能重复开始"
    );
    assert!(session.take_original().is_some());
    session.fail("已提交".to_owned());
    assert!(!session.restart_sentence());
}

#[test]
fn restart_requires_an_existing_translation_and_has_a_stable_action_name() {
    let mut session = strict_session("中文");
    assert!(!session.restart_sentence());
    assert_eq!(
        serde_json::to_string(&PracticeAction::Restart).unwrap(),
        "\"restart\""
    );
}
