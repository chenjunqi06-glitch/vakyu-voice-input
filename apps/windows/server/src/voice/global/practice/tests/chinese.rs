//! 中文跟读只在最终完整匹配后放行原英文，重试和词卡保留当前门禁。

use std::time::{Duration, Instant};

use qingjian_learning::{PracticeLevel, PracticeMatchConfig};

use crate::voice::global::practice::{PracticeSession, lookup};

fn session() -> PracticeSession {
    PracticeSession::new_for_language(
        "I want to eat today.".to_owned(),
        PracticeLevel::Newcomer,
        "0 / 120 XP".to_owned(),
        PracticeMatchConfig::default(),
        false,
    )
}

#[test]
fn chinese_reading_releases_original_english_only_after_final_and_keeps_pinyin() {
    let mut session = session();
    assert!(session.translated("我今天要吃饭。"));
    assert!(!session.view().learn_english);
    assert_eq!(
        session.view().words.len(),
        session.view().pronunciations.len()
    );
    assert_eq!(session.view().pronunciations[0], "wǒ");
    session.started();
    let now = Instant::now();
    session.interim("我今天", now);
    assert!(!session.should_check(now + Duration::from_secs(2)));
    assert!(session.completed_original().is_none());
    session.interim("我今天要吃饭", now);
    assert!(session.should_check(now + Duration::from_secs(1)));
    assert!(session.completed_original().is_none());
    session.checking();
    assert!(session.final_result("我今天要吃饭。"));
    assert_eq!(session.completed_original(), Some("I want to eat today."));
}

#[test]
fn chinese_wrong_final_can_resume_and_word_lookup_does_not_advance() {
    let mut session = session();
    assert!(session.translated("我今天要吃饭。"));
    session.started();
    session.interim("我今天要", Instant::now());
    session.checking();
    assert!(!session.final_result("我今天"));
    let matched = session.view().matched_words;
    assert_eq!(matched, 4);
    session.show_word_card(lookup::word_card_for_language("饭", false));
    assert_eq!(session.view().matched_words, matched);
    assert!(session.completed_original().is_none());
    session.close_word_card();
    session.started();
    session.interim("吃饭", Instant::now());
    session.checking();
    assert!(session.final_result("吃饭"));
    assert_eq!(session.completed_original(), Some("I want to eat today."));
}

#[test]
fn english_translation_cannot_bypass_chinese_reading() {
    let mut session = session();
    assert!(!session.translated("I want to eat today."));
    assert!(session.completed_original().is_none());
}
