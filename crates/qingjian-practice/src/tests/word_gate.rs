//! 累计通过仍保留未读词门禁；不会因重试增加弱读容错预算。

use std::time::Instant;

use qingjian_learning::{PracticeLevel, PracticeMatchConfig};

use crate::PracticeSession;

#[test]
fn repeated_updates_and_retries_share_one_weak_word_budget() {
    let mut session = PracticeSession::new(
        "中文原文".to_owned(),
        PracticeLevel::Newcomer,
        String::new(),
    );
    assert!(session.translated("I want to eat dinner and I plan to visit school."));
    session.started();
    let now = Instant::now();
    session.interim("I want eat dinner", now);
    assert_eq!(session.view().matched_words, 5);
    session.interim("I want eat dinner and I plan visit school", now);
    assert_eq!(session.view().matched_words, 8);
    session.checking();
    assert!(!session.final_result("I want eat dinner and I plan visit school"));
    assert!(session.take_original().is_none());
    session.started();
    session.interim("to visit school", now);
    session.checking();
    assert!(session.final_result("to visit school"));
}

#[test]
fn chinese_revisions_preserve_numeric_and_latin_name_units() {
    let mut session = PracticeSession::new_for_language(
        "John Smith is 25 this year.".to_owned(),
        PracticeLevel::Challenge,
        String::new(),
        PracticeMatchConfig::default(),
        false,
    );
    assert!(session.translated("John Smith今年25岁。"));
    session.started();
    let now = Instant::now();
    session.interim("John Smith今", now);
    assert_eq!(session.view().matched_words, 3);
    session.interim("John Smith斤", now);
    assert_eq!(session.view().matched_words, 3);
    session.interim("John Smith斤年二十五", now);
    assert_eq!(session.view().matched_words, 6);
    session.checking();
    assert!(!session.final_result("John Smith斤年二十五"));
    session.started();
    session.interim("岁", now);
    session.checking();
    assert!(session.final_result("岁"));
}

#[test]
fn long_repeated_target_still_needs_the_final_word() {
    let mut session = PracticeSession::new(
        "长句原文".to_owned(),
        PracticeLevel::Challenge,
        String::new(),
    );
    assert!(session.translated(&format!("{}finish", "we read words ".repeat(350))));
    session.started();
    let now = Instant::now();
    let first = "we read words ".repeat(175);
    session.interim(&first, now);
    assert_eq!(session.view().matched_words, 525);
    session.interim(&first, now);
    assert_eq!(session.view().matched_words, 525);
    let complete_prefix = "we read words ".repeat(350);
    session.interim(&complete_prefix, now);
    assert_eq!(session.view().matched_words, 1050);
    session.checking();
    assert!(!session.final_result(&complete_prefix));
    assert!(session.take_original().is_none());
    session.started();
    session.interim("finish", now);
    session.checking();
    assert!(session.final_result("finish"));
}
