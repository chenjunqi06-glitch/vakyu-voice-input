//! 专名的拼音音节与有限混淆；不能删除音节，也不能把任意英语单词视为人名。

use pinyin::ToPinyinMulti;
use qingjian_core::parser;

use crate::english_practice::level::PracticeLevel;
use crate::english_practice::similarity;

pub(super) fn latin(text: &str) -> Vec<Vec<String>> {
    if text.len() > 36 || !text.bytes().all(|ch| ch.is_ascii_lowercase()) {
        return Vec::new();
    }
    parser::segment(text)
        .unwrap_or_default()
        .into_iter()
        .filter(|segmentation| {
            segmentation
                .syllables
                .iter()
                .all(|syllable| syllable.complete)
                && segmentation.syllables.len() <= 6
        })
        .map(|segmentation| {
            segmentation
                .syllables
                .into_iter()
                .map(|syllable| syllable.text)
                .collect()
        })
        .collect()
}

pub(super) fn heard(text: &str) -> Vec<Vec<String>> {
    if text.is_ascii() {
        return latin(text);
    }
    let mut variants = vec![Vec::new()];
    for character in text.chars().take(7) {
        let Some(readings) = character.to_pinyin_multi() else {
            return Vec::new();
        };
        variants = variants
            .iter()
            .flat_map(|prefix| {
                readings.into_iter().map(move |reading| {
                    let mut candidate = prefix.clone();
                    candidate.push(reading.plain().to_owned());
                    candidate
                })
            })
            .filter(|candidate| candidate.len() <= 6)
            .take(64)
            .collect();
    }
    if text.chars().count() > 6 {
        Vec::new()
    } else {
        variants
    }
}

pub(super) fn compare(
    expected: &[String],
    heard: &[String],
    level: PracticeLevel,
    similarity_threshold: u8,
) -> Option<bool> {
    if expected.len() != heard.len() {
        return None;
    }
    if similarity::similarity_percent(&expected.join(" "), &heard.join(" "))
        <= similarity_threshold.min(100)
    {
        return None;
    }
    let mut total = 0;
    for (expected, heard) in expected.iter().zip(heard) {
        let cost = syllable_cost(expected, heard);
        if cost <= 2 {
            total += cost;
        } else if level == PracticeLevel::Newcomer
            && similarity::similarity_percent(expected, heard) > similarity_threshold.min(100)
        {
            total += 1;
        } else {
            return None;
        }
    }
    let limit = match level {
        PracticeLevel::Newcomer => expected.len(),
        PracticeLevel::Intermediate => (expected.len() / 3).max(1),
        PracticeLevel::Challenge => 0,
    };
    (total <= limit).then_some(total != 0)
}

fn syllable_cost(expected: &str, heard: &str) -> usize {
    if expected == heard {
        return 0;
    }
    let (left_initial, left_final) = split(expected);
    let (right_initial, right_final) = split(heard);
    let initial = if left_initial == right_initial {
        0
    } else if [
        &["j", "q", "x"][..],
        &["z", "zh"],
        &["c", "ch"],
        &["s", "sh"],
        &["n", "l"],
        &["f", "h"],
        &["l", "r"],
    ]
    .iter()
    .any(|group| group.contains(&left_initial) && group.contains(&right_initial))
    {
        1
    } else {
        10
    };
    let (left_vowel, left_coda) = final_parts(left_initial, left_final);
    let (right_vowel, right_coda) = final_parts(right_initial, right_final);
    let vowel = if left_vowel == right_vowel {
        0
    } else if matches!(
        (left_vowel.as_str(), right_vowel.as_str()),
        ("v", "i") | ("i", "v")
    ) {
        1
    } else {
        10
    };
    let coda = if left_coda == right_coda {
        0
    } else if matches!((left_coda, right_coda), ("n", "ng") | ("ng", "n")) {
        1
    } else {
        10
    };
    initial + vowel + coda
}

fn split(syllable: &str) -> (&str, &str) {
    for initial in [
        "zh", "ch", "sh", "b", "p", "m", "f", "d", "t", "n", "l", "g", "k", "h", "j", "q", "x",
        "r", "z", "c", "s", "y", "w",
    ] {
        if let Some(final_part) = syllable.strip_prefix(initial) {
            return (initial, final_part);
        }
    }
    ("", syllable)
}

fn final_parts<'a>(initial: &str, final_part: &'a str) -> (String, &'a str) {
    let (vowel, coda) = if let Some(vowel) = final_part.strip_suffix("ng") {
        (vowel, "ng")
    } else if let Some(vowel) = final_part.strip_suffix('n') {
        (vowel, "n")
    } else {
        (final_part, "")
    };
    let vowel = if matches!(initial, "j" | "q" | "x" | "y") && vowel == "u" {
        "v"
    } else {
        vowel
    };
    (vowel.to_owned(), coda)
}
