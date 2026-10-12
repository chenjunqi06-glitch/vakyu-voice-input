//! 中文学习词卡的本地英文义与带调拼音；多音字明确展示备选读音。

use pinyin::{ToPinyin, ToPinyinMulti};

use super::table;

/// 直接查询已有中英释义表；未收录条目不逐字拼凑成虚构的整词义。
pub fn offline_chinese_meaning(word: &str) -> Option<String> {
    let word = word.trim_matches(|character: char| !character.is_alphanumeric());
    if word.is_empty() || word.len() > 160 {
        return None;
    }
    table::lookup(table::CHINESE, word).and_then(table::display_senses)
}

/// 拼音完全本地生成；词语使用常见首读，单个多音字显示各读音供辨认。
pub fn chinese_pinyin(word: &str) -> Option<String> {
    let word = word.trim_matches(|character: char| !character.is_alphanumeric());
    if word.is_empty() || word.chars().count() > 80 {
        return None;
    }
    if let Some(known) = phrase_reading(word) {
        return Some(known.to_owned());
    }
    if word.chars().count() == 1 {
        let readings = word.chars().next()?.to_pinyin_multi()?;
        return Some(
            readings
                .into_iter()
                .map(|reading| reading.with_tone())
                .collect::<Vec<_>>()
                .join(" / "),
        );
    }
    word.chars()
        .map(|character| character.to_pinyin().map(|reading| reading.with_tone()))
        .collect::<Option<Vec<_>>>()
        .map(|readings| readings.join(" "))
}

/// 为逐字跟读生成一条主读音；常见多音词跨相邻显示单元修正，标点不占位置。
pub fn chinese_practice_readings(words: &[String]) -> Vec<String> {
    let tokens: Vec<String> = words
        .iter()
        .map(|word| {
            word.chars()
                .filter(|character| character.is_alphanumeric())
                .collect()
        })
        .collect();
    let mut readings: Vec<String> = tokens
        .iter()
        .map(|token| {
            token
                .chars()
                .map(|character| character.to_pinyin().map(|reading| reading.with_tone()))
                .collect::<Option<Vec<_>>>()
                .map(|syllables| syllables.join(" "))
                .unwrap_or_default()
        })
        .collect();
    for index in 0..tokens.len().saturating_sub(1) {
        // 前一单元已结束句子/分句时，不把两边的同形字连成词。
        if words[index]
            .chars()
            .any(|character| "，。！？；,.!?;".contains(character))
            || tokens[index].chars().count() != 1
            || tokens[index + 1].chars().count() != 1
        {
            continue;
        }
        if let Some(phrase) = phrase_reading(&format!("{}{}", tokens[index], tokens[index + 1])) {
            let syllables: Vec<&str> = phrase.split_whitespace().collect();
            if syllables.len() == 2 {
                readings[index] = syllables[0].to_owned();
                readings[index + 1] = syllables[1].to_owned();
            }
        }
    }
    readings
}

fn phrase_reading(word: &str) -> Option<&'static str> {
    match word {
        "银行" | "銀行" => Some("yín háng"),
        "行业" | "行業" => Some("háng yè"),
        "行走" => Some("xíng zǒu"),
        "行为" | "行為" => Some("xíng wéi"),
        "音乐" | "音樂" => Some("yīn yuè"),
        "快乐" | "快樂" => Some("kuài lè"),
        "长大" | "長大" => Some("zhǎng dà"),
        "长城" | "長城" => Some("cháng chéng"),
        "重新" => Some("chóng xīn"),
        "重复" | "重複" => Some("chóng fù"),
        "重庆" | "重慶" => Some("chóng qìng"),
        "重要" => Some("zhòng yào"),
        "觉得" | "覺得" => Some("jué de"),
        "睡觉" | "睡覺" => Some("shuì jiào"),
        "了解" => Some("liǎo jiě"),
        "还原" | "還原" => Some("huán yuán"),
        "还有" | "還有" => Some("hái yǒu"),
        "方便" => Some("fāng biàn"),
        "便宜" => Some("pián yi"),
        _ => None,
    }
}
