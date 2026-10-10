//! 本轮完整跟读、补读和识别反馈回归。

use super::{reading, strict_session};
use crate::dispatch::PracticeWordCard;
use crate::voice::global::practice::{Phase, PracticeSession};
use std::time::{Duration, Instant};

#[test]
fn incomplete_reordered_and_interim_only_speech_cannot_release_chinese() {
    let now = Instant::now();
    let mut session = reading();
    session.interim("I want eat rice today", now);
    assert!(!session.should_check(now + Duration::from_secs(3)));
    assert!(session.take_original().is_none());
    session.interim("I want to eat rice today", now);
    assert!(!session.should_check(now + Duration::from_millis(649)));
    assert!(session.should_check(now + Duration::from_millis(650)));
    assert!(session.take_original().is_none());
    session.checking();
    assert!(session.final_result("I want to eat rice today."));
    assert_eq!(session.completed_original(), Some("我今天想吃米饭。"));
    assert_eq!(session.take_original().as_deref(), Some("我今天想吃米饭。"));
    assert_eq!(session.completed_original(), None);
    assert!(session.take_original().is_none(), "不能重复提交");
}

#[test]
fn shorter_final_keeps_words_that_have_already_passed() {
    let now = Instant::now();
    let mut session = reading();
    session.interim("I want to eat rice today", now);
    session.checking();
    assert!(session.final_result("I want to eat rice"));
    assert_eq!(session.phase, Phase::Complete);
    assert_eq!(session.view().matched_words, 6);
    assert!(session.take_original().is_some());
}

#[test]
fn revised_interim_keeps_passed_highlight_but_still_waits_for_final() {
    let now = Instant::now();
    let mut session = reading();
    session.interim("I want to eat rice today", now);
    session.interim("I want to eat", now + Duration::from_millis(500));
    assert!(session.should_check(now + Duration::from_secs(10)));
    assert_eq!(session.view().matched_words, 6);
    assert!(session.take_original().is_none());
}

#[test]
fn beginner_can_correct_an_old_final_with_a_fresh_phrase() {
    let mut session = PracticeSession::new(
        "我今天晚饭想吃鸡肉".to_owned(),
        qingjian_learning::PracticeLevel::Newcomer,
        "0 / 120 XP".to_owned(),
    );
    assert!(session.translated("I want chicken for dinner today."));
    session.started();
    let now = Instant::now();
    session.interim("I want chicken", now);
    session.checking();
    assert!(!session.final_result("I want fish"));
    assert_eq!(session.view().matched_words, 3);
    assert!(session.take_original().is_none());
    session.started();
    session.interim("for dinner today", now);
    assert!(session.should_check(now + Duration::from_secs(1)));
    assert!(session.take_original().is_none());
    session.checking();
    assert!(session.final_result("for dinner today"));
    assert_eq!(
        session.take_original().as_deref(),
        Some("我今天晚饭想吃鸡肉")
    );
    assert!(session.take_original().is_none());
}

#[test]
fn a_full_reread_after_an_interrupted_wrong_attempt_can_still_release_original() {
    let mut session = PracticeSession::new(
        "我今天晚饭想吃鸡肉".to_owned(),
        qingjian_learning::PracticeLevel::Newcomer,
        "0 / 120 XP".to_owned(),
    );
    assert!(session.translated("I want chicken for dinner today."));
    session.started();
    let now = Instant::now();
    session.interim("I want fish for dinner today", now);
    session.checking();
    assert!(!session.final_result("I want fish for dinner today"));
    assert!(session.take_original().is_none());

    session.started();
    let replay = "Can you call me later? I want chicken for dinner today. Are you coming?";
    assert!(session.interim(replay, now + Duration::from_secs(1)));
    assert_eq!(session.view().matched_words, 6);
    assert!(session.should_check(now + Duration::from_secs(2)));
    session.checking();
    assert!(session.final_result(replay));
    assert_eq!(
        session.take_original().as_deref(),
        Some("我今天晚饭想吃鸡肉")
    );
    assert!(session.take_original().is_none());
}

#[test]
fn failure_and_unrequested_final_never_bypass_the_gate() {
    let mut session = strict_session("中文原文");
    assert!(!session.translated(""));
    assert!(session.take_original().is_none());
    let mut unsupported = strict_session("中文原文");
    assert!(!unsupported.translated("Please read 中文"));
    assert_eq!(unsupported.phase, Phase::Failed);
    let mut session = reading();
    assert!(!session.final_result("I want to eat rice today"));
    assert_eq!(session.phase, Phase::Failed);
    assert!(session.take_original().is_none());
}

#[test]
fn coalesced_started_and_unmatched_interim_still_show_recording_status() {
    let mut session = strict_session("中文");
    assert!(session.translated("I want to eat rice today."));
    assert!(session.interim("unrelated words", Instant::now()));
    assert_eq!(session.phase, Phase::Reading);
    assert_eq!(session.view().matched_words, 0);
    assert!(session.view().status.contains("正在录音"));
    assert!(session.take_original().is_none());
}

#[test]
fn changed_unmatched_speech_refreshes_feedback_without_unlocking_chinese() {
    let mut session = reading();
    let now = Instant::now();
    assert!(session.interim("one sound", now));
    assert!(session.interim("another sound", now));
    assert_eq!(session.view().matched_words, 0);
    assert_eq!(session.view().recognized, "another sound");
    assert!(!session.interim("another sound", now));
    assert!(session.take_original().is_none());
}

#[test]
fn beginner_name_can_be_split_or_recognized_as_chinese_but_needs_its_end() {
    for recognized in ["my name is chen jing xi", "my name is 陈景熙"] {
        let mut session = PracticeSession::new(
            "我叫陈君琪".to_owned(),
            qingjian_learning::PracticeLevel::Newcomer,
            "0 / 120 XP".to_owned(),
        );
        assert!(session.translated("My name is Chen Junqi."));
        assert_eq!(session.recognition_hint(), "My name is Chen Junqi.");
        session.started();
        let now = Instant::now();
        session.interim("my name is chen jing", now);
        assert!(!session.should_check(now + Duration::from_secs(2)));
        session.interim(recognized, now);
        assert!(
            session.should_check(now + Duration::from_secs(1)),
            "{recognized}"
        );
        assert!(session.take_original().is_none());
        session.checking();
        assert!(session.final_result(recognized), "{recognized}");
        assert_eq!(session.take_original().as_deref(), Some("我叫陈君琪"));
        assert!(session.take_original().is_none());
    }
}

#[test]
fn translation_timeout_is_separate_and_long_reading_has_no_time_cutoff() {
    let session = strict_session("中文");
    assert!(!session.timed_out(Instant::now() + Duration::from_secs(20)));
    assert!(session.timed_out(Instant::now() + Duration::from_secs(130)));
    let session = reading();
    assert!(!session.timed_out(Instant::now() + Duration::from_secs(3600)));
}

#[test]
fn demonstration_never_counts_as_reading_or_unlocks_the_original() {
    let mut session = strict_session("我想吃饭");
    assert!(session.translated("I want to eat."));
    session.demonstrating();
    assert_eq!(session.phase, Phase::Demonstrating);
    assert!(!session.interim("I want to eat", Instant::now()));
    assert_eq!(session.view().matched_words, 0);
    assert!(session.take_original().is_none());
    session.demo_finished(false);
    assert_eq!(session.phase, Phase::Preparing);
    session.started();
    assert_eq!(session.phase, Phase::Reading);
    assert!(session.take_original().is_none());
}

#[test]
fn beginner_tolerance_still_waits_for_the_final_user_transcript() {
    let mut session = PracticeSession::new(
        "我今天想吃晚饭".to_owned(),
        qingjian_learning::PracticeLevel::Newcomer,
        "0 / 120 XP".to_owned(),
    );
    assert!(session.translated("I want to eat dinner today."));
    session.started();
    let now = Instant::now();
    session.interim("I want eat dinner today", now);
    assert!(session.should_check(now + Duration::from_secs(1)));
    assert!(session.take_original().is_none());
    session.checking();
    assert!(session.final_result("I want eat dinner today"));
    assert!(session.progress().complete);
    assert!(session.take_original().is_some());
    assert!(session.take_original().is_none());
}

#[test]
fn word_lookup_and_speech_feedback_do_not_change_reading_progress() {
    let mut session = reading();
    session.interim("I want to eat", Instant::now());
    let matched = session.view().matched_words;
    let phase = session.phase;
    session.show_word_card(PracticeWordCard {
        word: "rice".to_owned(),
        meaning: Some("米饭".to_owned()),
        pronunciation: Some("/raɪs/".to_owned()),
        hint: None,
    });
    session.set_word_card_hint(Some("正在播放发音".to_owned()));
    assert_eq!(
        session.view().word_card.unwrap().hint.as_deref(),
        Some("正在播放发音")
    );
    assert_eq!(session.view().matched_words, matched);
    assert_eq!(session.phase, phase);
    session.close_word_card();
    assert_eq!(session.view().matched_words, matched);
    assert_eq!(session.phase, phase);
}

#[test]
fn failed_final_keeps_asr_feedback_and_exposes_retry_skip_and_manual_accept() {
    let mut session = reading();
    session.started();
    session.interim("I want fish", Instant::now());
    session.checking();
    assert!(!session.final_result("I want fish"));
    let view = session.view();
    assert_eq!(view.recognized, "I want fish");
    assert!(view.feedback.contains("fish"));
    assert!(view.feedback.contains("当前词是 to"));
    assert!(view.actions_enabled);
    assert!(session.take_original().is_none());

    assert!(session.retry_current());
    assert!(session.view().actions_enabled);
    assert!(session.view().recognized.is_empty());
    session.started();
    session.interim("I want fish", Instant::now());
    session.checking();
    assert!(!session.final_result("I want fish"));
    assert!(session.skip_current());
    assert_eq!(session.view().matched_words, 3);
    assert_eq!(session.view().feedback, "已跳过：to");
    session.started();
    session.interim("eat rice today", Instant::now());
    session.checking();
    assert!(session.final_result("eat rice today"));
    assert_eq!(session.take_original().as_deref(), Some("我今天想吃米饭。"));
}

#[test]
fn manual_accept_only_releases_the_current_word_and_never_skips_sentence_tail() {
    let mut session = reading();
    session.started();
    session.interim("I want to eat", Instant::now());
    session.checking();
    assert!(!session.final_result("I want to eat"));
    assert!(session.view().actions_enabled);
    assert!(session.accept_current());
    assert_eq!(session.view().matched_words, 5);
    assert!(!session.progress().complete);
    assert!(session.take_original().is_none());
    session.started();
    session.interim("today", Instant::now());
    session.checking();
    assert!(session.final_result("today"));
    assert!(session.take_original().is_some());
}

#[test]
fn manual_actions_are_available_while_waiting_for_a_word_during_recording() {
    let mut session = reading();
    assert!(session.view().actions_enabled);
    session.interim("I want to eat", Instant::now());
    assert!(session.accept_current());
    assert_eq!(session.view().matched_words, 5);
    assert!(session.take_original().is_none());
}

#[test]
fn pronunciation_pause_resumes_at_the_existing_prefix() {
    let mut session = reading();
    session.interim("I want to eat", Instant::now());
    session.pause_for_pronunciation();
    assert_eq!(session.view().matched_words, 4);
    session.started();
    session.interim("rice today", Instant::now());
    session.checking();
    assert!(session.final_result("rice today"));
}

#[test]
fn failed_insertion_can_retry_without_skipping_beyond_the_last_word() {
    let mut session = reading();
    session.checking();
    assert!(session.final_result("I want to eat rice today"));
    session.fail("输入框暂不可用".to_owned());
    assert!(!session.skip_current());
    assert!(session.retry_current());
    assert_eq!(session.completed_original(), Some("我今天想吃米饭。"));
}

#[test]
#[ignore = "需要已生成的英文 ASR 回放结果；不打开麦克风、不填入应用"]
fn actual_english_asr_replay_obeys_final_gate() {
    let path = std::env::var("QINGJIAN_PRACTICE_ASR_FIXTURE").expect("已生成英文回放 JSON");
    let data = std::fs::read_to_string(path).unwrap();
    let fixtures: Vec<serde_json::Value> = serde_json::from_str(&data).unwrap();
    assert!(fixtures.len() >= 6);
    for fixture in fixtures {
        let original = "这是必须等跟读完成才能填入的中文。";
        let mut session = strict_session(original);
        assert!(session.translated(fixture["expected"].as_str().unwrap()));
        session.started();
        let start = Instant::now();
        let mut last = start;
        for update in fixture["interim"].as_array().unwrap() {
            last = start + Duration::from_secs_f64(update["audio_seconds"].as_f64().unwrap());
            session.interim(update["text"].as_str().unwrap(), last);
            assert!(session.take_original().is_none(), "临时结果不能上屏");
        }
        assert!(session.should_check(last + Duration::from_secs(1)));
        session.checking();
        assert!(session.final_result(fixture["final"].as_str().unwrap()));
        assert_eq!(session.take_original().as_deref(), Some(original));
        assert!(session.take_original().is_none());
    }
}
