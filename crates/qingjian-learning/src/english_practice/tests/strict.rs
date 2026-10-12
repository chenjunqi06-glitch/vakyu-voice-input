//! 跟读门控的回归测试：遗漏、临时结果回改与数字不能被错误接受。

use crate::english_practice::EnglishPractice;

#[test]
fn advances_only_through_the_ordered_prefix() {
    let practice = EnglishPractice::new("I want to eat dinner today.");
    assert_eq!(practice.evaluate("I want").matched_words, 2);
    assert_eq!(practice.evaluate("eat dinner today").matched_words, 0);
    assert_eq!(
        practice.evaluate("I want eat dinner today").matched_words,
        2
    );
    assert!(!practice.evaluate("I want eat dinner today").complete);
    assert!(practice.evaluate("I want to eat dinner today").complete);
}

#[test]
fn complete_transcript_revisions_can_remove_progress() {
    let practice = EnglishPractice::new("We will meet tomorrow.");
    assert!(practice.evaluate("we will meet tomorrow").complete);
    let corrected = practice.evaluate("we will eat tomorrow");
    assert_eq!(corrected.matched_words, 2);
    assert!(!corrected.complete);
    assert_eq!(practice.evaluate("").matched_words, 0);
}

#[test]
fn tolerates_fillers_and_repeated_attempts_without_skipping_targets() {
    let practice = EnglishPractice::new("I need two tickets.");
    assert!(
        practice
            .evaluate("um I I need uh two two tickets please")
            .complete
    );
    assert!(!practice.evaluate("I need three tickets").complete);
    assert!(!practice.evaluate("tickets two need I").complete);
}

#[test]
fn repeated_target_words_need_separate_recognized_occurrences() {
    let practice = EnglishPractice::new("Very very good.");
    assert_eq!(practice.evaluate("very good").matched_words, 1);
    assert!(practice.evaluate("very very good").complete);
}

#[test]
fn keeps_display_words_but_ignores_case_and_punctuation() {
    let practice = EnglishPractice::new("Hello, WORLD! Well-known.");
    assert_eq!(practice.words(), &["Hello,", "WORLD!", "Well-known."]);
    assert!(practice.evaluate("hello world well known").complete);
    assert_eq!(practice.evaluate("hello world well").matched_words, 2);
    assert!(!EnglishPractice::new("eat").evaluate("eating").complete);
}

#[test]
fn unambiguous_contractions_match_in_both_directions() {
    let practice = EnglishPractice::new("I’m ready. We can't wait; you'll love it.");
    assert!(
        practice
            .evaluate("I am ready we cannot wait you will love it")
            .complete
    );
    assert!(
        EnglishPractice::new("We are ready and do not worry.")
            .evaluate("we're ready and don't worry")
            .complete
    );
    assert_eq!(
        EnglishPractice::new("I'm ready")
            .evaluate("I")
            .matched_words,
        0
    );
}

#[test]
fn numbers_match_spoken_cardinals_without_accepting_other_values() {
    let practice = EnglishPractice::new("I need 25 tickets and 1,000 dollars.");
    assert!(
        practice
            .evaluate("I need twenty five tickets and one thousand dollars")
            .complete
    );
    assert!(
        !practice
            .evaluate("I need 26 tickets and 1000 dollars")
            .complete
    );
    assert!(
        EnglishPractice::new("The value is 12.50.")
            .evaluate("the value is twelve point five zero")
            .complete
    );
    assert!(
        EnglishPractice::new("The code is 007.")
            .evaluate("the code is zero zero seven")
            .complete
    );
}

#[test]
fn empty_punctuation_or_untranslated_chinese_never_unlocks() {
    for target in ["", "  ", "... !?", "今天吃饭", "I 今天"] {
        assert!(!EnglishPractice::new(target).evaluate(target).complete);
    }
}

#[test]
fn latin_accents_match_conservatively_in_both_directions() {
    let practice = EnglishPractice::new("My résumé is at the CAFÉ.");
    assert!(practice.is_valid());
    assert!(practice.evaluate("my resume is at the cafe").complete);
    assert!(
        EnglishPractice::new("My resume is at the cafe.")
            .evaluate("my résumé is at the café")
            .complete
    );
    assert!(
        practice
            .evaluate("my re\u{0301}sume\u{0301} is at the cafe\u{0301}")
            .complete
    );
    assert!(
        EnglishPractice::new("naïve façade")
            .evaluate("naive facade")
            .complete
    );
}

#[test]
fn unsupported_scripts_are_invalid_and_never_silently_dropped() {
    for target in ["你好", "Привет", "καλημέρα", "مرحبا", "I 你好", "I ١٢٣"]
    {
        let practice = EnglishPractice::new(target);
        assert!(!practice.is_valid(), "{target}");
        assert!(!practice.evaluate(target).complete, "{target}");
        assert!(!practice.evaluate("I").complete, "{target}");
    }
    assert!(EnglishPractice::new("I need help.").is_valid());
    assert!(!EnglishPractice::new("...").is_valid());
}

#[test]
fn a_long_target_requires_its_last_word_and_retains_the_whole_target() {
    let target = format!(
        "{}finish.",
        "I will keep practicing every day. ".repeat(180)
    );
    let practice = EnglishPractice::new(&target);
    assert_eq!(practice.words().len(), 1081);
    let without_tail = "I will keep practicing every day. ".repeat(180);
    let progress = practice.evaluate(&without_tail);
    assert_eq!(progress.matched_words, 1080);
    assert!(!progress.complete);
    assert!(practice.evaluate(&target).complete);
}
