//! 离线词义查找、词形回退、拼音与二分边界回归。

use super::{
    chinese_pinyin, chinese_practice_readings, chinese_practice_word, forms,
    offline_chinese_meaning, offline_english_meaning, table,
};

#[test]
fn offline_lookup_is_case_insensitive_and_trims_punctuation() {
    assert_eq!(
        offline_english_meaning("HELLO,").as_deref(),
        Some("int. 你好")
    );
    assert_eq!(
        offline_english_meaning("people.").as_deref(),
        Some("n. 人们")
    );
}

#[test]
fn offline_unknown_and_non_word_input_have_no_guess() {
    assert_eq!(offline_english_meaning("not-in-this-dictionary"), None);
    assert_eq!(offline_english_meaning("..."), None);
    assert_eq!(offline_chinese_meaning("这是一个故意不存在的词条"), None);
}

#[test]
fn offline_chinese_uses_full_existing_dictionary() {
    assert!(offline_chinese_meaning("你好！").is_some_and(|meaning| meaning.contains("hello")));
    assert!(
        offline_chinese_meaning("光合作用")
            .is_some_and(|meaning| meaning.contains("photosynthesis"))
    );
    assert!(offline_chinese_meaning("我").is_some());
}

#[test]
fn offline_forms_preserve_known_base_words() {
    assert!(forms::base_forms("technologist's").contains(&"technologist".to_owned()));
    assert!(forms::base_forms("debugging").contains(&"debug".to_owned()));
    assert!(forms::base_forms("dictionaries").contains(&"dictionary".to_owned()));
    assert!(forms::base_forms("studied").contains(&"study".to_owned()));
    assert!(offline_english_meaning("technologist's").is_some());
}

#[test]
fn offline_missing_english_entry_uses_existing_reverse_glossary() {
    assert_eq!(table::lookup(table::ENGLISH, "multimeter"), None);
    assert!(
        offline_english_meaning("multimeter").is_some_and(|meaning| meaning.contains("万用表"))
    );
    assert!(
        offline_english_meaning("multimeters").is_some_and(|meaning| meaning.contains("万用表"))
    );
    assert_eq!(
        offline_english_meaning("multimeter"),
        offline_english_meaning("multimeter")
    );
}

#[test]
fn offline_pinyin_is_local_with_tones_and_polyphonic_choices() {
    assert_eq!(chinese_pinyin("你好，").as_deref(), Some("nǐ hǎo"));
    assert_eq!(chinese_pinyin("银行").as_deref(), Some("yín háng"));
    assert!(
        chinese_pinyin("行")
            .is_some_and(|reading| reading.contains("xíng") && reading.contains("háng"))
    );
    assert_eq!(chinese_pinyin("hello"), None);
}

#[test]
fn offline_inline_readings_use_one_tone_and_phrase_context() {
    let words: Vec<String> = [
        "“银", "行，", "音", "乐。", "重", "庆！", "和", "Alice", "。",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(
        chinese_practice_readings(&words),
        ["yín", "háng", "yīn", "yuè", "chóng", "qìng", "hé", "", ""]
    );
}

#[test]
fn offline_clicked_chinese_character_expands_known_words_without_crossing_punctuation() {
    let bank: Vec<String> = ["去", "银", "行，"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    assert_eq!(chinese_practice_word(&bank, 1).as_deref(), Some("银行"));
    assert_eq!(chinese_practice_word(&bank, 2).as_deref(), Some("银行"));
    let sentence: Vec<String> = "我今天要吃饭"
        .chars()
        .map(|character| character.to_string())
        .collect();
    assert_eq!(chinese_practice_word(&sentence, 0).as_deref(), Some("我"));
    assert_eq!(chinese_practice_word(&sentence, 1).as_deref(), Some("今天"));
    assert_eq!(chinese_practice_word(&sentence, 4).as_deref(), Some("吃饭"));
    assert_eq!(chinese_practice_word(&sentence, 5).as_deref(), Some("吃饭"));
    let separated: Vec<String> = ["银，", "行", "吃。", "饭"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    assert_eq!(chinese_practice_word(&separated, 0).as_deref(), Some("银"));
    assert_eq!(chinese_practice_word(&separated, 1).as_deref(), Some("行"));
    assert_eq!(chinese_practice_word(&separated, 2).as_deref(), Some("吃"));
    assert_eq!(chinese_practice_word(&separated, 3).as_deref(), Some("饭"));
    assert_eq!(chinese_practice_word(&bank, 9), None);
}

#[test]
fn offline_clicked_greetings_are_real_words_but_personal_fragments_are_not() {
    for greeting in ["你好", "您好"] {
        let words: Vec<String> = greeting
            .chars()
            .map(|character| character.to_string())
            .collect();
        assert_eq!(chinese_practice_word(&words, 0).as_deref(), Some(greeting));
        assert_eq!(chinese_practice_word(&words, 1).as_deref(), Some(greeting));
    }
    let fragment = vec!["我".to_owned(), "今".to_owned()];
    assert_eq!(chinese_practice_word(&fragment, 0).as_deref(), Some("我"));
}

#[test]
fn offline_binary_search_handles_utf8_first_last_and_missing_keys() {
    let table = "# source\n你\thello\n好\tgood\n我\tI\n";
    assert_eq!(table::lookup(table, "你"), Some("hello"));
    assert_eq!(table::lookup(table, "我"), Some("I"));
    assert_eq!(table::lookup(table, "啊"), None);
    assert_eq!(table::lookup(table, "龟"), None);
    assert_eq!(table::lookup("a\tone\nb\ttwo", "b"), Some("two"));
    for data in [table::ENGLISH, table::CHINESE] {
        for (index, line) in data
            .lines()
            .filter(|line| !line.starts_with('#'))
            .enumerate()
        {
            if index % 997 == 0 {
                let (word, senses) = line.split_once('\t').unwrap();
                assert_eq!(table::lookup(data, word), Some(senses), "{word}");
            }
        }
    }
}
