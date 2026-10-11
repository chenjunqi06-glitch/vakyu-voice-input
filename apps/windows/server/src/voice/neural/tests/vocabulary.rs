//! 普通口述提示的范围、配置开关与缓存重载；只读写测试目录，不访问真实用户词表或麦克风。

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

use crate::voice::neural::command::HelperCommand;
use crate::voice::neural::vocabulary::{
    BASE_WORDS, DictationVocabulary, MAX_BYTES, MAX_CONFIG_BYTES, MAX_PERSONAL_BYTES,
    MAX_PERSONAL_WORDS, MAX_WORDS,
};

fn with_directory(test: impl FnOnce(&Path)) {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "vakyu-dictation-vocabulary-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| test(&root)));
    std::fs::remove_dir_all(&root).unwrap();
    if let Err(error) = result {
        std::panic::resume_unwind(error);
    }
}

fn command(name: &'static str, practice: Option<&str>) -> HelperCommand {
    HelperCommand {
        command: name,
        generation: 9,
        language: Some("zh"),
        practice_text: practice.map(str::to_owned),
        dictation_vocabulary: None,
    }
}

fn vocabulary(loader: &mut DictationVocabulary) -> String {
    let mut start = command("start", None);
    loader.apply(&mut start);
    start.dictation_vocabulary.unwrap()
}

#[test]
fn missing_and_empty_user_data_preserve_only_the_validated_base() {
    assert_eq!(vocabulary(&mut DictationVocabulary::new(None)), BASE_WORDS);
    assert_eq!(BASE_WORDS.split_whitespace().count(), 69);
    with_directory(|root| {
        let mut loader = DictationVocabulary::new(Some(root.to_owned()));
        assert_eq!(vocabulary(&mut loader), BASE_WORDS);
        std::fs::write(root.join("config.toml"), "").unwrap();
        std::fs::write(root.join("user-english.tsv"), "# header\n").unwrap();
        assert_eq!(vocabulary(&mut loader), BASE_WORDS);
    });
}

#[test]
fn personal_words_merge_case_variants_and_follow_frequency() {
    with_directory(|root| {
        std::fs::write(
            root.join("user-english.tsv"),
            "Zephyr\t3\nALPHA\t4\nzephyr\t4\nDocker\t5\nAPI\t200\n",
        )
        .unwrap();
        let actual = vocabulary(&mut DictationVocabulary::new(Some(root.to_owned())));
        assert_eq!(actual, format!("{BASE_WORDS} Zephyr Docker ALPHA"));
    });
}

#[test]
fn only_bounded_english_tokens_are_accepted() {
    with_directory(|root| {
        let mut source =
            "hello@example.com\t999\n中文\t999\nx\t999\n'bad\t999\nrock'n'roll\t999\ncan't\t5\n"
                .to_owned();
        source.push_str(&format!("{}\t999\n", "x".repeat(49)));
        for index in 0..50_u8 {
            let word = format!(
                "word{}{}",
                char::from(b'a' + index / 26),
                char::from(b'a' + index % 26)
            );
            source.push_str(&format!("{word}\t1\n"));
        }
        std::fs::write(root.join("user-english.tsv"), source).unwrap();
        let actual = vocabulary(&mut DictationVocabulary::new(Some(root.to_owned())));
        let added: Vec<_> = actual
            .strip_prefix(BASE_WORDS)
            .unwrap()
            .split_whitespace()
            .collect();
        assert_eq!(added.len(), MAX_PERSONAL_WORDS);
        assert_eq!(added[0], "can't");
        assert!(added[1..].iter().all(|word| word.starts_with("word")));
        assert!(actual.split_whitespace().count() <= MAX_WORDS);
        assert!(actual.len() <= MAX_BYTES);
    });
}

#[test]
fn disabled_or_invalid_configuration_removes_personal_words_next_round() {
    with_directory(|root| {
        std::fs::write(root.join("user-english.tsv"), "Zephyr\t9\n").unwrap();
        let mut loader = DictationVocabulary::new(Some(root.to_owned()));
        assert!(vocabulary(&mut loader).ends_with(" Zephyr"));
        for source in [
            "[general]\nlearning = false\n",
            "[general]\nlearning = \"true\"\n",
            "[general\nlearning = true\n",
            "general = 42\n",
        ] {
            std::fs::write(root.join("config.toml"), source).unwrap();
            assert_eq!(vocabulary(&mut loader), BASE_WORDS);
        }
        std::fs::write(root.join("config.toml"), "[general]\nlearning = true\n").unwrap();
        assert!(vocabulary(&mut loader).ends_with(" Zephyr"));
        std::fs::write(root.join("config.toml"), [0xff, 0xfe]).unwrap();
        assert_eq!(vocabulary(&mut loader), BASE_WORDS);
    });
}

#[test]
fn oversized_or_unreadable_files_fail_closed_for_personal_words() {
    with_directory(|root| {
        std::fs::write(root.join("user-english.tsv"), "Zephyr\t9\n").unwrap();
        let mut loader = DictationVocabulary::new(Some(root.to_owned()));
        std::fs::write(root.join("config.toml"), vec![b' '; MAX_CONFIG_BYTES + 1]).unwrap();
        assert_eq!(vocabulary(&mut loader), BASE_WORDS);
        std::fs::remove_file(root.join("config.toml")).unwrap();
        std::fs::create_dir(root.join("config.toml")).unwrap();
        assert_eq!(vocabulary(&mut loader), BASE_WORDS);
        std::fs::remove_dir(root.join("config.toml")).unwrap();
        for damaged in [
            b"Zephyr\tbroken\n".to_vec(),
            vec![0xff],
            vec![b'x'; MAX_PERSONAL_BYTES + 1],
        ] {
            std::fs::write(root.join("user-english.tsv"), damaged).unwrap();
            assert_eq!(vocabulary(&mut loader), BASE_WORDS);
        }
        std::fs::remove_file(root.join("user-english.tsv")).unwrap();
        std::fs::create_dir(root.join("user-english.tsv")).unwrap();
        assert_eq!(vocabulary(&mut loader), BASE_WORDS);
    });
}

#[test]
fn changed_metadata_reloads_same_length_words_and_missing_data_clears_cache() {
    with_directory(|root| {
        let path = root.join("user-english.tsv");
        let mut loader = DictationVocabulary::new(Some(root.to_owned()));
        for (revision, text, expected) in
            [(1, "Zephyr\t9\n", " Zephyr"), (2, "Docker\t9\n", " Docker")]
        {
            std::fs::write(&path, text).unwrap();
            set_modified(&path, revision);
            let first = vocabulary(&mut loader);
            assert!(first.ends_with(expected));
            assert_eq!(vocabulary(&mut loader), first);
        }
        std::fs::remove_file(path).unwrap();
        assert_eq!(vocabulary(&mut loader), BASE_WORDS);
    });
}

fn set_modified(path: &Path, revision: u64) {
    std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000 + revision))
        .unwrap();
}

#[test]
fn only_ordinary_start_serializes_vocabulary_and_never_reuses_it_for_control() {
    let mut loader = DictationVocabulary::new(None);
    for (name, practice) in [
        ("start", None),
        ("start", Some("")),
        ("start", Some("My name is Chen.")),
        ("stop", None),
        ("cancel", None),
        ("shutdown", None),
    ] {
        let mut current = command(name, practice);
        current.dictation_vocabulary = Some("stale personal text".to_owned());
        loader.apply(&mut current);
        let value = serde_json::to_value(current).unwrap();
        if name == "start" && practice.is_none() {
            assert_eq!(value["dictation_vocabulary"], BASE_WORDS);
        } else {
            assert!(value.get("dictation_vocabulary").is_none());
        }
        assert_eq!(value["generation"], 9);
    }
}
