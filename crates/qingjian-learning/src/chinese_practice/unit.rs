//! 中文识别单元与有限音近规则；不按单个汉字编辑距离放行任意替换。

use pinyin::ToPinyinMulti;

use super::{latin, number};
use crate::{PracticeLevel, PracticeMatchConfig};

#[derive(Clone, Debug)]
pub(super) struct Unit {
    pub(super) display_words: usize,

    normalized: String,

    readings: Vec<&'static str>,
}

impl Unit {
    fn new(text: &str) -> Self {
        let normalized = latin::normalize(text);
        let mut characters = text.chars();
        let first = characters.next();
        let readings = if characters.next().is_none() {
            first
                .and_then(|character| character.to_pinyin_multi())
                .map(|readings| {
                    readings
                        .into_iter()
                        .map(|reading| reading.plain())
                        .collect()
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        Self {
            display_words: 1,
            normalized,
            readings,
        }
    }

    pub(super) fn matches(
        &self,
        heard: &Self,
        level: PracticeLevel,
        config: PracticeMatchConfig,
    ) -> bool {
        if self.normalized == heard.normalized {
            return true;
        }
        let expected_digit = digit_reading(&self.normalized);
        let heard_digit = digit_reading(&heard.normalized);
        let expected = self.readings.iter().copied().chain(expected_digit);
        for left in expected {
            let heard_readings = heard.readings.iter().copied().chain(heard_digit);
            for right in heard_readings {
                if left == right {
                    return true;
                }
                // 只接受已知平翘舌/前后鼻音差异，防止短拼音的编辑距离过度宽松。
                if level != PracticeLevel::Challenge
                    && config.word_similarity < 85
                    && near_syllable(left, right)
                {
                    return true;
                }
            }
        }
        false
    }
}

pub(super) fn is_han(character: char) -> bool {
    matches!(character, '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}'
        | '\u{f900}'..='\u{faff}' | '\u{20000}'..='\u{323af}')
}

pub(super) fn tokenize(text: &str) -> (Vec<String>, Vec<Unit>) {
    let mut words: Vec<String> = Vec::new();
    let mut units = Vec::new();
    let mut latin = String::new();
    let mut leading = String::new();
    let characters: Vec<char> = text.chars().collect();
    let mut index = 0;
    while index < characters.len() {
        let character = characters[index];
        if latin::letter(character)
            || (matches!(character, '\'' | '\u{300}'..='\u{36f}') && !latin.is_empty())
        {
            latin.push(character);
            index += 1;
            continue;
        }
        flush_latin(&mut latin, &mut leading, &mut words, &mut units);
        if let Some((count, canonical)) = number::take(&characters, index) {
            let display_words = characters[index..index + count]
                .iter()
                .filter(|character| !character.is_whitespace())
                .count();
            units.push(Unit {
                display_words,
                normalized: format!("number:{canonical}"),
                readings: Vec::new(),
            });
            for character in characters[index..index + count]
                .iter()
                .filter(|character| !character.is_whitespace())
            {
                words.push(format!("{}{character}", std::mem::take(&mut leading)));
            }
            index += count;
            continue;
        }
        if character.is_alphanumeric() {
            let token = character.to_string();
            units.push(Unit::new(&token));
            words.push(format!("{}{token}", std::mem::take(&mut leading)));
        } else if !character.is_whitespace() {
            if let Some(previous) = words.last_mut() {
                previous.push(character);
            } else {
                leading.push(character);
            }
        }
        index += 1;
    }
    flush_latin(&mut latin, &mut leading, &mut words, &mut units);
    (words, units)
}

fn flush_latin(
    latin: &mut String,
    leading: &mut String,
    words: &mut Vec<String>,
    units: &mut Vec<Unit>,
) {
    if !latin.is_empty() {
        units.push(Unit::new(latin));
        words.push(format!(
            "{}{}",
            std::mem::take(leading),
            std::mem::take(latin)
        ));
    }
}

fn digit_reading(value: &str) -> Option<&'static str> {
    match value {
        "0" => Some("ling"),
        "1" => Some("yi"),
        "2" => Some("er"),
        "3" => Some("san"),
        "4" => Some("si"),
        "5" => Some("wu"),
        "6" => Some("liu"),
        "7" => Some("qi"),
        "8" => Some("ba"),
        "9" => Some("jiu"),
        _ => None,
    }
}

fn near_syllable(left: &str, right: &str) -> bool {
    for (long, short) in [("zh", "z"), ("ch", "c"), ("sh", "s")] {
        if left
            .strip_prefix(long)
            .zip(right.strip_prefix(short))
            .is_some_and(|(a, b)| a == b)
            || right
                .strip_prefix(long)
                .zip(left.strip_prefix(short))
                .is_some_and(|(a, b)| a == b)
        {
            return true;
        }
    }
    for (long, short) in [("eng", "en"), ("ing", "in")] {
        if left
            .strip_suffix(long)
            .zip(right.strip_suffix(short))
            .is_some_and(|(a, b)| a == b)
            || right
                .strip_suffix(long)
                .zip(left.strip_suffix(short))
                .is_some_and(|(a, b)| a == b)
        {
            return true;
        }
    }
    false
}
