//! 从点击的汉字就近扩展真实词条；不跨标点、跳字或把人称句子片段当作词。

use super::table;

/// 返回包含点击位置的最长 2～4 字本地词条，找不到时保留当前汉字。
pub fn chinese_practice_word(words: &[String], index: usize) -> Option<String> {
    let current = single_han(words.get(index)?)?;
    for length in (2..=4).rev() {
        let earliest = index.saturating_sub(length - 1);
        for start in (earliest..=index).rev() {
            let end = start + length;
            if end > words.len() || end <= index {
                continue;
            }
            let Some(candidate) = span(words, start, end) else {
                continue;
            };
            if let Some(senses) = table::lookup(table::CHINESE, &candidate)
                && lexical_entry(&candidate, senses)
            {
                return Some(candidate);
            }
        }
    }
    Some(current.to_string())
}

fn span(words: &[String], start: usize, end: usize) -> Option<String> {
    let mut result = String::new();
    for (offset, word) in words[start..end].iter().enumerate() {
        let character = single_han(word)?;
        let position = word.find(character)?;
        // 显示标点可附着前一个字或第一个字；只禁止跨词条内部的边界。
        if (offset > 0 && !word[..position].trim().is_empty())
            || (offset + 1 < end - start
                && !word[position + character.len_utf8()..].trim().is_empty())
        {
            return None;
        }
        result.push(character);
    }
    Some(result)
}

fn single_han(word: &str) -> Option<char> {
    let mut characters = word.chars().filter(|character| character.is_alphanumeric());
    let character = characters.next()?;
    if characters.next().is_some()
        || !matches!(character, '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}'
            | '\u{f900}'..='\u{faff}' | '\u{20000}'..='\u{323af}')
    {
        return None;
    }
    Some(character)
}

fn lexical_entry(word: &str, senses: &str) -> bool {
    // 原表包含“我今”之类机器生成句子片段；仅人称代词本身允许作多字词。
    if word.starts_with(['我', '你', '他', '她', '它', '您', '咱'])
        && !matches!(
            word,
            "我们" | "你们" | "他们" | "她们" | "它们" | "您们" | "咱们" | "你好" | "您好"
        )
    {
        return false;
    }
    senses.split('\t').any(|sense| {
        let (part_of_speech, meaning) = table::split_sense(sense);
        part_of_speech != "phr"
            && !meaning.is_empty()
            && ![
                "want to ", "need to ", "will ", "can ", "I ", "you ", "we ", "he ", "she ",
                "they ",
            ]
            .iter()
            .any(|prefix| meaning.starts_with(prefix))
    })
}
