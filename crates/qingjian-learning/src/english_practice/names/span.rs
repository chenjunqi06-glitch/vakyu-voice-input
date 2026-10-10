//! 一段已由原文确认的拼音专名；拆词和汉字识别都必须覆盖同样多的连续音节。

use crate::english_practice::level::PracticeLevel;

use super::syllables;

#[derive(Clone, Debug)]
pub(in crate::english_practice) struct NameSpan {
    pub start: usize,

    pub end: usize,

    pub syllables: Vec<String>,
}

impl NameSpan {
    pub fn matches(
        &self,
        recognized: &[String],
        cursor: usize,
        level: PracticeLevel,
        similarity_threshold: u8,
    ) -> Option<(usize, bool)> {
        let mut prefixes = vec![Vec::new()];
        for (end, word) in recognized.iter().enumerate().skip(cursor).take(6) {
            let options = syllables::heard(word);
            if options.is_empty() {
                break;
            }
            prefixes = prefixes
                .iter()
                .flat_map(|prefix| {
                    options.iter().map(move |option| {
                        let mut combined = prefix.clone();
                        combined.extend(option.iter().cloned());
                        combined
                    })
                })
                .filter(|value| value.len() <= self.syllables.len())
                .take(64)
                .collect();
            if prefixes.is_empty() {
                break;
            }
            for candidate in &prefixes {
                if let Some(similar) =
                    syllables::compare(&self.syllables, candidate, level, similarity_threshold)
                {
                    return Some((end + 1, similar));
                }
            }
        }
        None
    }
}
