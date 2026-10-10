//! 常见口音、拆词与重读的验收；读音相近需要真实词，不能靠缺失或无关文本放行。

use crate::english_practice::{EnglishPractice, PracticeLevel, PracticeMatchConfig};

#[test]
fn newcomer_accepts_several_explainable_accent_changes_without_omitting_anything() {
    let practice =
        EnglishPractice::with_level("Thank you very much today.", PracticeLevel::Newcomer);
    let result = practice.evaluate_details("tank you wery much to day");
    assert!(result.progress.complete, "{result:?}");
    assert_eq!(result.omitted_words, 0);
    assert_eq!(result.tolerated_words, 3);
    for (expected, heard) in [
        ("Thank you", "tank you"),
        ("Very good", "wery good"),
        ("I think", "I sink"),
        ("This is fine", "dis is fine"),
        ("I am with you", "I am wif you"),
    ] {
        assert!(
            EnglishPractice::with_level(expected, PracticeLevel::Newcomer)
                .evaluate(heard)
                .complete,
            "{expected}: {heard}"
        );
    }
}

#[test]
fn actual_homophones_and_word_splitting_are_valid_at_all_levels() {
    for level in [
        PracticeLevel::Newcomer,
        PracticeLevel::Intermediate,
        PracticeLevel::Challenge,
    ] {
        for (expected, heard) in [
            ("Today is fine", "to day is fine"),
            ("I see", "I sea"),
            ("We meet now", "we meat now"),
        ] {
            assert!(
                EnglishPractice::with_level(expected, level)
                    .evaluate(heard)
                    .complete,
                "{level:?}: {expected}: {heard}"
            );
        }
    }
}

#[test]
fn accepted_accent_forms_can_be_repeated_and_used_to_correct_earlier_attempts() {
    let practice =
        EnglishPractice::with_level("I think about dinner today.", PracticeLevel::Newcomer);
    for heard in [
        "I sink sink about dinner today",
        "I fish sink about dinner today",
        "I sink about dinner today sink",
        "I sink about dinner today I sink about dinner today",
    ] {
        assert!(practice.evaluate(heard).complete, "{heard}");
    }
    let expected = "I would like to visit the beautiful park tomorrow after I finish working";
    assert!(
        EnglishPractice::with_level(expected, PracticeLevel::Newcomer)
            .evaluate(&format!("{expected} {expected}"))
            .complete
    );
}

#[test]
fn phoneme_matching_cannot_drop_numbers_negatives_syllables_or_unrelated_content() {
    for level in [
        PracticeLevel::Newcomer,
        PracticeLevel::Intermediate,
        PracticeLevel::Challenge,
    ] {
        for (expected, heard) in [
            ("I need three tickets", "I need tree tickets"),
            ("I do not want dinner", "I do want dinner"),
            ("I want dinner", "I do not want dinner"),
            ("I have no money", "I have know money"),
            ("I eat dinner", "I eating dinner"),
            ("I want chicken", "I want fish"),
            ("Thank you very much", "tank you"),
            ("Very very good", "wery good"),
            ("Tomorrow is fine", "to is fine"),
            (
                "I think about dinner",
                "I think unrelated about bananas dinner",
            ),
        ] {
            assert!(
                !EnglishPractice::with_level(expected, level)
                    .evaluate(heard)
                    .complete,
                "{level:?}: {expected}: {heard}"
            );
        }
    }
}

#[test]
fn chinese_source_confirms_joined_and_split_names_without_encoding_any_person() {
    for expected in ["My name is Chen Junqi.", "My name is Chen Jun Qi."] {
        let practice =
            EnglishPractice::with_context(expected, "我叫陈君琪", PracticeLevel::Newcomer);
        for heard in [
            "my name is chen jun qi",
            "my name is chen jing xi",
            "my name is 陈君琪",
            "my name is 陈景熙",
            "my name is陈君琪",
            "my name is陈景熙",
        ] {
            assert!(practice.evaluate(heard).complete, "{expected}: {heard}");
        }
        for heard in [
            "my name is chen jing",
            "my name is 陈景",
            "我叫陈君琪",
            "my name is wang xiao ming",
            "my name is chen xi jing",
        ] {
            assert!(!practice.evaluate(heard).complete, "{expected}: {heard}");
        }
    }
    let other = EnglishPractice::with_context(
        "I am Zhang Xiaoming.",
        "我是张小明",
        PracticeLevel::Newcomer,
    );
    assert!(other.evaluate("I am zhang xiao ming").complete);
    assert!(other.evaluate("I am 张小明").complete);
    assert!(!other.evaluate("I am 张小").complete);
}

#[test]
fn name_rules_require_source_context_and_become_stricter_with_level() {
    let expected = "I am Chen Junqi.";
    for practice in [
        EnglishPractice::with_level(expected, PracticeLevel::Newcomer),
        EnglishPractice::with_context(expected, "我叫王小明", PracticeLevel::Newcomer),
        EnglishPractice::with_context(expected, "我叫陈君琪", PracticeLevel::Challenge),
    ] {
        assert!(!practice.evaluate("I am chen jing xi").complete);
    }
    let exact = EnglishPractice::with_context(expected, "我叫陈君琪", PracticeLevel::Challenge);
    assert!(exact.evaluate("I am chen jun qi").complete);
    assert!(exact.evaluate("I am 陈君琪").complete);
    let ordinary = EnglishPractice::with_context(
        "Can a man help me",
        "一个人可以帮我吗",
        PracticeLevel::Newcomer,
    );
    assert!(!ordinary.evaluate("kan a nan help me").complete);
}

#[test]
fn configurable_fuzzy_threshold_accepts_small_asr_spelling_changes() {
    let lenient = EnglishPractice::with_level_and_config(
        "My name is beautiful.",
        PracticeLevel::Newcomer,
        PracticeMatchConfig {
            word_similarity: 80,
            name_similarity: 45,
        },
    );
    assert!(lenient.evaluate("MY, name is beautifull!").complete);

    let stricter = EnglishPractice::with_level_and_config(
        "My name is beautiful.",
        PracticeLevel::Newcomer,
        PracticeMatchConfig {
            word_similarity: 90,
            name_similarity: 45,
        },
    );
    assert!(!stricter.evaluate("my name is beautifull").complete);
}

#[test]
fn chinese_confirmed_proper_name_uses_its_lower_configured_threshold() {
    let lenient = EnglishPractice::with_context_and_config(
        "My name is Chen Junqi.",
        "我叫陈君琪",
        PracticeLevel::Newcomer,
        PracticeMatchConfig {
            word_similarity: 78,
            name_similarity: 45,
        },
    );
    assert!(lenient.evaluate("my name is chen jing xi").complete);

    let stricter = EnglishPractice::with_context_and_config(
        "My name is Chen Junqi.",
        "我叫陈君琪",
        PracticeLevel::Newcomer,
        PracticeMatchConfig {
            word_similarity: 78,
            name_similarity: 80,
        },
    );
    assert!(!stricter.evaluate("my name is chen jing xi").complete);
}
