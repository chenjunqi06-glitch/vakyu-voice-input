//! 只为中文原句能逐字确认的专名启用拼音；普通英文不进入这条容错路径。

mod span;
mod syllables;

use pinyin::ToPinyinMulti;

pub(super) use span::NameSpan;

pub(super) fn confirm(words: &[String], units: &[Vec<String>], original: &str) -> Vec<NameSpan> {
    let source: Vec<Option<Vec<&str>>> = original
        .chars()
        .map(|ch| {
            ch.to_pinyin_multi()
                .map(|values| values.into_iter().map(|p| p.plain()).collect())
        })
        .collect();
    let mut spans = Vec::new();
    let mut start = 0;
    while start < words.len() {
        let mut best = None;
        let mut joined = String::new();
        for end in start..words.len().min(start + 4) {
            // 大写专名加源中文逐字对应双重确认；can/man 等普通英文不能借拼音放宽。
            if !words[end].chars().next().is_some_and(char::is_uppercase)
                || units[end].len() != 1
                || !units[end][0].chars().all(|ch| ch.is_ascii_alphabetic())
            {
                break;
            }
            joined.push_str(&units[end][0]);
            if let Some(syllables) = syllables::latin(&joined).into_iter().find(|syllables| {
                (2..=6).contains(&syllables.len())
                    && source.windows(syllables.len()).any(|characters| {
                        characters
                            .iter()
                            .zip(syllables)
                            .all(|(readings, expected)| {
                                readings
                                    .as_ref()
                                    .is_some_and(|readings| readings.contains(&expected.as_str()))
                            })
                    })
            }) {
                best = Some(NameSpan {
                    start,
                    end: end + 1,
                    syllables,
                });
            }
            if words[end].ends_with(['.', '!', '?', ';', ':']) {
                break;
            }
        }
        if let Some(span) = best {
            start = span.end;
            spans.push(span);
        } else {
            start += 1;
        }
    }
    spans
}
