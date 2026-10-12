//! 英文词义先查精确词条与词形，再反查本地中英词库；96 项结果缓存限制内存。

use std::{collections::VecDeque, sync::Mutex};

use super::{forms, table};

static CACHE: Mutex<VecDeque<(String, Option<String>)>> = Mutex::new(VecDeque::new());
const CACHE_LIMIT: usize = 96;

/// 查英文词的中文释义；词形回退标出原形，未知词不联网或猜测。
pub fn offline_english_meaning(word: &str) -> Option<String> {
    let word = forms::normalize(word);
    if word.is_empty() || word.len() > 160 {
        return None;
    }
    if let Ok(cache) = CACHE.lock()
        && let Some((_, meaning)) = cache.iter().find(|(entry, _)| entry == &word)
    {
        return meaning.clone();
    }
    let result = lookup(&word);
    if let Ok(mut cache) = CACHE.lock() {
        if cache.len() == CACHE_LIMIT {
            cache.pop_front();
        }
        cache.push_back((word, result.clone()));
    }
    result
}

fn lookup(word: &str) -> Option<String> {
    if let Some(senses) = table::lookup(table::ENGLISH, word) {
        return table::display_senses(senses);
    }
    let bases = forms::base_forms(word);
    for base in &bases {
        if let Some(senses) = table::lookup(table::ENGLISH, base)
            && let Some(meaning) = table::display_senses(senses)
        {
            return Some(format!("{meaning}（词形：{base}）"));
        }
    }
    reverse_lookup(word, &bases)
}

fn reverse_lookup(word: &str, bases: &[String]) -> Option<String> {
    let mut candidates: Vec<(usize, String)> = Vec::new();
    for line in table::CHINESE.lines().filter(|line| !line.starts_with('#')) {
        let Some((chinese, senses)) = line.split_once('\t') else {
            continue;
        };
        for sense in senses.split('\t') {
            let (pos, english) = table::split_sense(sense);
            let exact = english.eq_ignore_ascii_case(word);
            let base = if exact {
                None
            } else {
                bases.iter().find(|base| english.eq_ignore_ascii_case(base))
            };
            if !exact && base.is_none() {
                continue;
            }
            // 短中文词优先，避免把包含英文的解释片段错误当作词义。
            let rank = chinese.chars().count() + if exact { 0 } else { 100 };
            let mut meaning = if pos.is_empty() {
                chinese.to_owned()
            } else {
                format!("{pos}. {chinese}")
            };
            if let Some(base) = base {
                meaning.push_str(&format!("（词形：{base}）"));
            }
            if candidates.iter().any(|(_, existing)| existing == &meaning) {
                continue;
            }
            candidates.push((rank, meaning));
            candidates
                .sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
            candidates.truncate(3);
        }
    }
    (!candidates.is_empty()).then(|| {
        candidates
            .into_iter()
            .map(|(_, meaning)| meaning)
            .collect::<Vec<_>>()
            .join("；")
    })
}
