//! 已排序 TSV 的字节范围二分查找，直接借用编译期词表，无堆分配索引。

use std::cmp::Ordering;

pub(super) const ENGLISH: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/glossary/glossary-zh.tsv"
));

pub(super) const CHINESE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/glossary/glossary-en.tsv"
));

pub(super) fn lookup<'a>(table: &'a str, word: &str) -> Option<&'a str> {
    let mut low = 0;
    while table[low..].starts_with('#') {
        low += table[low..].find('\n')? + 1;
    }
    let mut high = table.len();
    while low < high {
        let midpoint = low + (high - low) / 2;
        // 先按字节寻找换行，再切 UTF-8；midpoint 可能落在汉字中间。
        let start = table.as_bytes()[low..midpoint]
            .iter()
            .rposition(|byte| *byte == b'\n')
            .map_or(low, |offset| low + offset + 1);
        let end = table.as_bytes()[start..high]
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(high, |offset| start + offset);
        let line = table[start..end].trim_end_matches('\r');
        let Some((entry, senses)) = line.split_once('\t') else {
            low = (end + 1).min(high);
            continue;
        };
        match entry.cmp(word) {
            Ordering::Less => low = (end + 1).min(high),
            Ordering::Equal => return Some(senses),
            Ordering::Greater => high = start,
        }
    }
    None
}

pub(super) fn display_senses(senses: &str) -> Option<String> {
    let result = senses
        .split('\t')
        .map(|sense| sense.split('|').next().unwrap_or_default().trim())
        .filter(|sense| !sense.is_empty())
        .take(3)
        .collect::<Vec<_>>()
        .join("；");
    (!result.is_empty()).then_some(result)
}

pub(super) fn split_sense(sense: &str) -> (&str, &str) {
    let sense = sense.split('|').next().unwrap_or_default().trim();
    if let Some((pos, definition)) = sense.split_once(". ")
        && pos.len() <= 7
        && pos.chars().all(|character| character.is_ascii_alphabetic())
    {
        return (pos, definition.trim());
    }
    ("", sense)
}
