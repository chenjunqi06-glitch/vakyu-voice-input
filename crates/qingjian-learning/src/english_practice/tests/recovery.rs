//! 与 Session 累计文本一致的纠错回归：旧错误保留时仍能正确补读，必读词不可绕过。

use crate::english_practice::{EnglishPractice, PracticeLevel};

const LEVELS: [PracticeLevel; 3] = [
    PracticeLevel::Newcomer,
    PracticeLevel::Intermediate,
    PracticeLevel::Challenge,
];

#[test]
fn wrong_final_then_new_correct_phrase_recovers_from_the_accumulated_text() {
    for level in LEVELS {
        let practice = EnglishPractice::with_level("I want chicken for dinner today.", level);
        let previous_final = "I want fish";
        assert_eq!(practice.evaluate(previous_final).matched_words, 2);
        let new_interim = format!("{previous_final} chicken for dinner");
        assert_eq!(practice.evaluate(&new_interim).matched_words, 5);
        assert!(!practice.evaluate(&new_interim).complete);
        let new_final = format!("{previous_final} chicken for dinner today");
        assert!(practice.evaluate(&new_final).complete);
    }
}

#[test]
fn full_restart_can_recover_an_old_error_without_erasing_that_error() {
    for level in LEVELS {
        let practice = EnglishPractice::with_level("I want chicken for dinner today.", level);
        for transcript in [
            "I want fish for dinner today I want chicken for dinner today",
            "you want fish for dinner I want chicken for dinner today",
            "I want fish for lunch tomorrow sorry chicken for dinner today",
        ] {
            assert!(
                practice.evaluate(transcript).complete,
                "{level:?}: {transcript}"
            );
        }
    }
}

#[test]
fn a_contiguous_full_replay_can_ignore_conversation_before_and_after_it() {
    for level in LEVELS {
        let practice = EnglishPractice::with_level("I want chicken for dinner today.", level);
        assert!(practice
            .evaluate(
                "I want fish for dinner today. Anyway, can you call me later? I want chicken for dinner today."
            )
            .complete);
        assert!(!practice
            .evaluate("I want fish for dinner today. We should talk. I want chicken for dinner tonight.")
            .complete);
        assert!(
            !practice
                .evaluate("I want to call you later. We ate chicken after dinner today.")
                .complete
        );
    }
}

#[test]
fn a_word_and_its_following_phrase_can_be_recovered_after_long_conversation() {
    for level in LEVELS {
        let mid_sentence =
            EnglishPractice::with_level("I want chicken for dinner today and go home now.", level);
        assert_eq!(mid_sentence.evaluate("I want fish").matched_words, 2);
        assert!(mid_sentence
            .evaluate("I want fish. Let me tell you about the meeting tomorrow afternoon; chicken for dinner today and go home now. Can you call me later?")
            .complete);

        let final_word = EnglishPractice::with_level("I want chicken for dinner today.", level);
        assert_eq!(
            final_word
                .evaluate("I want chicken for dinner tonight")
                .matched_words,
            5
        );
        assert!(final_word
            .evaluate("I want chicken for dinner tonight. My friend is asking what time we should leave; today.")
            .complete);
    }
}

#[test]
fn stutters_and_fillers_do_not_have_a_fixed_count_limit() {
    let repetitions = "I ".repeat(40);
    let fillers = "um ".repeat(40);
    for level in LEVELS {
        let practice = EnglishPractice::with_level("I want chicken for dinner today.", level);
        assert!(
            practice
                .evaluate(&format!(
                    "{repetitions}want {fillers}chicken for dinner today"
                ))
                .complete
        );
    }
}

#[test]
fn corrected_numbers_negation_and_final_words_must_really_be_spoken() {
    for level in LEVELS {
        for (expected, correct, incomplete) in [
            (
                "We need two tickets tomorrow",
                "We need three two tickets tomorrow",
                "We need three tickets tomorrow",
            ),
            (
                "I do not want dinner today",
                "I do want not want dinner today",
                "I do want dinner today",
            ),
            (
                "I want chicken for dinner today",
                "I want chicken for dinner tonight today",
                "I want chicken for dinner tonight",
            ),
        ] {
            let practice = EnglishPractice::with_level(expected, level);
            assert!(practice.evaluate(correct).complete, "{level:?}: {correct}");
            assert!(
                !practice.evaluate(incomplete).complete,
                "{level:?}: {incomplete}"
            );
        }
    }
}

#[test]
fn recovery_does_not_assemble_a_sentence_out_of_unrelated_scattered_words() {
    for level in LEVELS {
        let practice = EnglishPractice::with_level("I want chicken for dinner today", level);
        for transcript in [
            "I saw bananas want apples chicken music for flowers dinner books today",
            "asdf I zxcv want chicken noise for dinner today",
            "I want fish. I need to talk to you. Chicken? Tomorrow dinner.",
            "I want fish for dinner today",
            "today dinner for chicken want I",
            "I want fish chicken",
        ] {
            assert!(
                !practice.evaluate(transcript).complete,
                "{level:?}: {transcript}"
            );
        }
    }
}
