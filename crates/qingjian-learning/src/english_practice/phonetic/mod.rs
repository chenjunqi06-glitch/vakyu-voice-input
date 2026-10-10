//! 基于英语发音词典的近音比较；每次匹配必须消耗实际识别词，不能省略目标。

mod distance;
mod lexicon;

use super::{level::PracticeLevel, tolerance};

pub(super) fn warm_up() {
    lexicon::warm_up();
}

pub fn ipa_variants(word: &str) -> Option<Vec<String>> {
    lexicon::ipa_variants(word)
}

pub(super) fn similar(expected: &str, heard: &[String], level: PracticeLevel) -> bool {
    if heard.is_empty()
        || heard.len() > 3
        || tolerance::protected(expected)
        || heard.iter().any(|word| tolerance::protected(word))
    {
        return false;
    }
    let expected_variants = lexicon::pronunciations(expected, false);
    if expected_variants.is_empty() {
        return false;
    }
    if let [word] = heard {
        let actual = lexicon::pronunciations(word, level == PracticeLevel::Newcomer);
        return expected_variants.iter().any(|expected| {
            actual
                .iter()
                .any(|heard| distance::similar(expected, heard, level))
        });
    }
    let mut actual_variants = vec![Vec::new()];
    for word in heard {
        let variants = lexicon::pronunciations(word, level == PracticeLevel::Newcomer);
        if variants.is_empty() {
            return false;
        }
        actual_variants = actual_variants
            .iter()
            .flat_map(|prefix| {
                variants.iter().map(move |phones| {
                    let mut combined = prefix.clone();
                    combined.extend(phones.iter().copied());
                    combined
                })
            })
            .take(24)
            .collect();
    }
    expected_variants.iter().any(|expected| {
        actual_variants
            .iter()
            .any(|heard| distance::similar(expected, heard, level))
    })
}
