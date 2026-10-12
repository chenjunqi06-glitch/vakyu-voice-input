//! 分级放行仍要求全部主要内容；容错只能修正有限的识别拼写与弱读。

use crate::english_practice::{EnglishPractice, PracticeLevel};

const LEVELS: [PracticeLevel; 3] = [
    PracticeLevel::Newcomer,
    PracticeLevel::Intermediate,
    PracticeLevel::Challenge,
];

#[test]
fn newcomer_can_continue_after_one_anchored_weak_word() {
    let expected = "I want to eat dinner today.";
    let practice = EnglishPractice::with_level(expected, PracticeLevel::Newcomer);
    let partial = practice.evaluate_details("I want eat");
    assert_eq!(partial.progress.matched_words, 4);
    assert_eq!(partial.omitted_words, 1);
    assert_eq!(partial.tolerated_words, 1);
    assert!(!partial.progress.complete);
    assert!(practice.evaluate("I want eat dinner today").complete);
    assert!(
        !EnglishPractice::new(expected)
            .evaluate("I want eat dinner today")
            .complete
    );
}

#[test]
fn weak_words_need_content_anchors_and_cannot_be_at_either_end() {
    for (expected, transcript) in [
        ("I want to eat dinner today", "I want"),
        ("I want to eat dinner today", "I want dinner today"),
        (
            "The children enjoy warm soup today",
            "children enjoy warm soup today",
        ),
        ("This is the place I went to", "This is the place I went"),
        ("We want to the market today", "We want market today"),
        ("I want to eat", "I want eat"),
    ] {
        let practice = EnglishPractice::with_level(expected, PracticeLevel::Newcomer);
        assert!(
            !practice.evaluate(transcript).complete,
            "{expected}: {transcript}"
        );
    }
    let pending =
        EnglishPractice::with_level("I want to eat dinner today", PracticeLevel::Newcomer);
    assert_eq!(pending.evaluate("I want").matched_words, 2);
}

#[test]
fn phoneme_substitutions_tighten_with_level_but_homophones_remain_valid() {
    let expected = "I think about dinner today.";
    let heard = "I sink about dinner today.";
    let beginner =
        EnglishPractice::with_level(expected, PracticeLevel::Newcomer).evaluate_details(heard);
    assert!(beginner.progress.complete);
    assert_eq!(beginner.tolerated_words, 1);
    assert_eq!(beginner.omitted_words, 0);
    for level in [PracticeLevel::Intermediate, PracticeLevel::Challenge] {
        assert!(
            !EnglishPractice::with_level(expected, level)
                .evaluate(heard)
                .complete
        );
    }
    for level in LEVELS {
        assert!(
            EnglishPractice::with_level("We meet our friends tomorrow", level)
                .evaluate("we meat our friends tomorrow")
                .complete
        );
    }
}

#[test]
fn short_targets_and_boundary_words_accept_real_homophones_without_omitting_words() {
    for level in LEVELS {
        for (expected, heard) in [
            ("we meet now", "we meat now"),
            ("Meet our friends here today", "Meat our friends here today"),
            ("I want to see", "I want to sea"),
        ] {
            assert!(
                EnglishPractice::with_level(expected, level)
                    .evaluate(heard)
                    .complete
            );
        }
        assert!(
            !EnglishPractice::with_level("I like to eat dinner today", level)
                .evaluate("I like to eating dinner today")
                .complete
        );
    }
    assert!(
        EnglishPractice::with_level("think", PracticeLevel::Newcomer)
            .evaluate("sink")
            .complete
    );
    for level in [PracticeLevel::Intermediate, PracticeLevel::Challenge] {
        assert!(
            !EnglishPractice::with_level("think", level)
                .evaluate("sink")
                .complete
        );
    }
}

#[test]
fn negatives_numbers_content_and_order_are_required_at_every_level() {
    for level in LEVELS {
        for (expected, heard) in [
            ("I do not want dinner today", "I do want dinner today"),
            ("I have no money for dinner", "I have know money for dinner"),
            (
                "We need 25 tickets tomorrow",
                "We need twenty six tickets tomorrow",
            ),
            (
                "We need three tickets tomorrow",
                "We need tree tickets tomorrow",
            ),
            ("I want rice and soup today", "I want rice soup today"),
            (
                "I want chicken for dinner today",
                "I want fish for dinner today",
            ),
            ("I want dinner today", "today dinner want I"),
            ("I want dinner today", "I want"),
            ("I want dinner today", "asdf I zxcv want dinner today"),
        ] {
            assert!(
                !EnglishPractice::with_level(expected, level)
                    .evaluate(heard)
                    .complete,
                "{level:?}: {expected}: {heard}"
            );
        }
        assert!(
            EnglishPractice::with_level("I want dinner today", level)
                .evaluate("你好 I want 然后 dinner today 为什么")
                .complete
        );
        assert!(
            EnglishPractice::with_level("I need 105 tickets tomorrow", level)
                .evaluate("I need one hundred and five tickets tomorrow")
                .complete
        );
    }
}

#[test]
fn mixed_language_asr_noise_does_not_block_an_otherwise_correct_reading() {
    let expected = "Can you make this UI not show up? If it is useless, it is best to delete it.";
    let transcript = "show up. 然后 Can you make this? Can you make this UI not show up? If it is useless, it is best to delete it. 为什么";
    for level in LEVELS {
        let progress = EnglishPractice::with_level(expected, level).evaluate(transcript);
        assert!(progress.complete, "{level:?}: {progress:?}");
        assert_eq!(progress.matched_words, progress.total_words);
    }
}

#[test]
fn fillers_restarts_and_contractions_preserve_ordered_progress() {
    for level in LEVELS {
        let practice = EnglishPractice::with_level("I'm ready and we need two tickets.", level);
        assert!(
            practice
                .evaluate("um I I am ready I am ready and we need uh two two tickets")
                .complete
        );
        assert!(
            !practice
                .evaluate("I am ready and we need three tickets")
                .complete
        );
        let revised = practice.evaluate("I am ready and we want two tickets");
        assert_eq!(revised.matched_words, 4);
        assert!(!revised.complete);
    }
}

#[test]
fn intermediate_has_smaller_omission_budget_and_challenge_has_none() {
    let expected = "I want to eat dinner today and I plan to visit the beautiful park tomorrow while we enjoy warm sunshine beside green trees and walk back home before it gets dark";
    let heard = "I want eat dinner today and I plan to visit the beautiful park tomorrow while we enjoy warm sunshine beside green trees and walk back home before it gets dark";
    assert_eq!(expected.split_whitespace().count(), 31);
    let intermediate =
        EnglishPractice::with_level(expected, PracticeLevel::Intermediate).evaluate_details(heard);
    assert!(intermediate.progress.complete);
    assert_eq!(intermediate.omitted_words, 1);
    assert!(
        !EnglishPractice::with_level(expected, PracticeLevel::Challenge)
            .evaluate(heard)
            .complete
    );
    assert!(
        !EnglishPractice::with_level(expected, PracticeLevel::Intermediate)
            .evaluate(&heard.replace("visit the beautiful", "visit beautiful"))
            .complete
    );
}

#[test]
fn long_target_keeps_tail_gate_and_linear_scan_limits() {
    let expected = format!("{}finish", "I want to eat dinner today. ".repeat(180));
    let heard = format!("{}finish", "I want to eat dinner today. ".repeat(180));
    for level in LEVELS {
        let practice = EnglishPractice::with_level(&expected, level);
        assert_eq!(practice.words().len(), 1081);
        assert!(practice.evaluate(&heard).complete);
        assert!(
            !practice
                .evaluate(heard.strip_suffix("finish").unwrap())
                .complete
        );
        assert!(!practice.evaluate(&"unrelated ".repeat(20_000)).complete);
    }
}
