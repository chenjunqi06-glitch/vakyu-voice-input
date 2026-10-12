//! 中文跟读的长句、重试、同音容错与未读文字保护。

use crate::{ChinesePractice, PracticeLevel, PracticeMatchConfig};

fn practice(expected: &str) -> ChinesePractice {
    ChinesePractice::with_level_and_config(
        expected,
        PracticeLevel::Newcomer,
        PracticeMatchConfig::default(),
    )
}

#[test]
fn chinese_practice_preserves_punctuation_but_ignores_asr_spacing() {
    let matcher = practice("“你好，世界！”");
    assert_eq!(matcher.words(), ["“你", "好，", "世", "界！”"]);
    assert!(matcher.evaluate("你 好。世界").complete);
}

#[test]
fn chinese_practice_accepts_homophones_and_traditional_asr() {
    assert!(practice("我想学习中文").evaluate("我想學習中文").complete);
    assert!(
        practice("我的名字是陈俊奇")
            .evaluate("我的名字是陈俊棋")
            .complete
    );
}

#[test]
fn chinese_practice_resumes_after_pause_chatter_and_rereading() {
    let matcher = practice("我今天要去吃饭");
    assert_eq!(matcher.evaluate("我今天").matched_words, 3);
    assert_eq!(
        matcher
            .evaluate("我今天，等等你先说，我今天要去")
            .matched_words,
        5
    );
    assert!(
        matcher
            .evaluate("我今天，等等你先说，我今天要去。要去吃饭")
            .complete
    );
}

#[test]
fn chinese_practice_cannot_skip_unread_target_or_invent_a_second_character() {
    assert!(!practice("我今天要去吃饭").evaluate("我今天去吃饭").complete);
    assert!(!practice("明天去北京").evaluate("今天去上海").complete);
    assert!(!practice("人人").evaluate("人").complete);
}

#[test]
fn chinese_practice_handles_thousand_characters_without_truncation() {
    let sentence = "我今天要去吃饭，明天下午三点去拿快递。".repeat(70);
    let matcher = practice(&sentence);
    assert!(matcher.words().len() > 1000);
    assert!(matcher.evaluate(&sentence).complete);
    assert!(!matcher.evaluate("我今天要去吃饭").complete);
}

#[test]
fn chinese_practice_accent_tolerance_is_configurable_and_bounded() {
    assert!(practice("中国").evaluate("宗国").complete);
    let strict = ChinesePractice::with_level_and_config(
        "中国",
        PracticeLevel::Newcomer,
        PracticeMatchConfig {
            word_similarity: 90,
            ..PracticeMatchConfig::default()
        },
    );
    assert!(!strict.evaluate("宗国").complete);
    assert!(!practice("中国").evaluate("美国").complete);
}

#[test]
fn chinese_practice_mixed_names_and_digits_are_not_discarded() {
    assert!(
        practice("我叫 Alice，下午3点见")
            .evaluate("我叫alice下午三点见")
            .complete
    );
    assert!(!practice("我叫 Alice").evaluate("我叫").complete);
    assert!(!practice("Hello!").is_valid());
    assert!(!practice("……").evaluate("你好").complete);
}

#[test]
fn chinese_practice_latin_names_fold_accents_and_keep_display_words() {
    let matcher = practice("我叫 José，我认识 Müller 和 Zoë。");
    assert!(matcher.words().iter().any(|word| word == "José，"));
    assert!(matcher.evaluate("我叫JOSE我认识Muller和Zoe").complete);
    assert!(practice("我叫 Jose").evaluate("我叫JOSÉ").complete);
    assert!(practice("我叫 José").evaluate("我叫Jose\u{301}").complete);
    assert!(!practice("我叫 José").evaluate("我叫John").complete);
}

#[test]
fn chinese_practice_numeric_age_time_quantity_and_years_are_equivalent() {
    assert!(
        practice("我三十岁，下午十二点带一百零二本书")
            .evaluate("我30岁下午12点带102本书")
            .complete
    );
    assert!(practice("我今年30岁").evaluate("我今年三十岁").complete);
    assert!(
        practice("二零二六年十月九日")
            .evaluate("2026年10月9日")
            .complete
    );
    assert!(
        practice("一百零二本书")
            .evaluate("一 百 零 二 本 书")
            .complete
    );
    assert!(!practice("我三十岁").evaluate("我3010岁").complete);
    assert!(!practice("我三十岁").evaluate("我三岁").complete);
}

#[test]
fn chinese_practice_numbers_keep_phone_digits_leading_zero_and_decimal_precision() {
    assert!(
        practice("电话零三零一二三四五六七八")
            .evaluate("电话03012345678")
            .complete
    );
    assert!(
        !practice("电话零三零一二三四五六七八")
            .evaluate("电话3012345678")
            .complete
    );
    assert!(practice("数值三点五零").evaluate("数值3.50").complete);
    assert!(practice("数值3.50").evaluate("数值三 点 五 零").complete);
    assert!(!practice("数值三点五零").evaluate("数值3.5").complete);
    assert!(!practice("数值三点五").evaluate("数值35").complete);
    assert!(!practice("数量一百二十").evaluate("数量一百二").complete);
}
