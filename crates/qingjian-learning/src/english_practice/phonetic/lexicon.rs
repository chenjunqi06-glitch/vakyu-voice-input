//! 只读 CMU 发音表；紧凑行偏移与有界小缓存避免全词典展开及反复解析。

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, LazyLock, Mutex};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Phone {
    pub symbol: &'static str,

    pub stress: u8,
}

type Phones = Vec<Phone>;
type Variants = Arc<[Phones]>;
type PronunciationCache = (HashMap<String, [Option<Variants>; 2]>, VecDeque<String>);

const CACHE_WORD_LIMIT: usize = 128;
const CACHE_KEY_BYTE_LIMIT: usize = 96;

// 只留最近插入的少量词；命中复用只读读音，不在每次 ASR 比较时深拷贝。
static CACHE: LazyLock<Mutex<PronunciationCache>> =
    LazyLock::new(|| Mutex::new((HashMap::new(), VecDeque::new())));

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/pronunciation/cmudict.dict"
));

static WORDS: LazyLock<Box<[u32]>> = LazyLock::new(|| {
    assert!(u32::try_from(SOURCE.len()).is_ok());
    let mut offsets = Vec::new();
    let mut offset = 0;
    for line in SOURCE.split_inclusive('\n') {
        if !line.trim().is_empty() && !line.starts_with('#') {
            offsets.push(offset as u32);
        }
        offset += line.len();
    }
    // 按去掉异体编号后的词排序，保留同词读音在原表中的先后顺序。
    offsets.sort_by(|left, right| entry_word(*left).cmp(entry_word(*right)));
    offsets.into_boxed_slice()
});

fn entry_line(offset: u32) -> &'static str {
    SOURCE[offset as usize..].split('\n').next().unwrap_or("")
}

fn entry_word(offset: u32) -> &'static str {
    entry_line(offset)
        .split_whitespace()
        .next()
        .unwrap_or("")
        .split('(')
        .next()
        .unwrap_or("")
}

fn lookup(word: &str) -> Vec<Phones> {
    let start = WORDS.partition_point(|offset| entry_word(*offset) < word);
    let mut result = Vec::new();
    for offset in WORDS[start..].iter().copied() {
        if entry_word(offset) != word {
            break;
        }
        let phones: Phones = entry_line(offset)
            .split_whitespace()
            .skip(1)
            .take_while(|phone| *phone != "#")
            .map(parse_phone)
            .collect();
        if !phones.is_empty() && !result.contains(&phones) {
            result.push(phones);
        }
    }
    result
}

pub(super) fn warm_up() {
    LazyLock::force(&WORDS);
}

pub(super) fn pronunciations(word: &str, allow_spelling: bool) -> Variants {
    let cacheable = word.len() <= CACHE_KEY_BYTE_LIMIT;
    if cacheable {
        let cache = CACHE.lock().unwrap_or_else(|error| error.into_inner());
        if let Some(variants) = cache
            .0
            .get(word)
            .and_then(|entry| entry[usize::from(allow_spelling)].as_ref())
        {
            return Arc::clone(variants);
        }
    }
    // 解析在锁外完成；两个线程恰好查询同词时，插入阶段仍共用第一份结果。
    let phones = lookup(word);
    let exact = !phones.is_empty();
    let variants = if exact || !allow_spelling {
        Arc::from(phones)
    } else {
        Arc::from(spelling_variants(word))
    };
    if !cacheable {
        return variants;
    }
    let mut cache = CACHE.lock().unwrap_or_else(|error| error.into_inner());
    cache_variants(&mut cache, word, allow_spelling, exact, variants)
}

fn cache_variants(
    cache: &mut PronunciationCache,
    word: &str,
    allow_spelling: bool,
    exact: bool,
    variants: Variants,
) -> Variants {
    let (entries, order) = cache;
    if entries.len() >= CACHE_WORD_LIMIT
        && !entries.contains_key(word)
        && let Some(oldest) = order.pop_front()
    {
        entries.remove(&oldest);
    }
    let entry = entries.entry(word.to_owned()).or_insert_with(|| {
        order.push_back(word.to_owned());
        [None, None]
    });
    let slot = usize::from(allow_spelling);
    if let Some(existing) = &entry[slot] {
        return Arc::clone(existing);
    }
    if exact {
        *entry = [Some(Arc::clone(&variants)), Some(Arc::clone(&variants))];
    } else {
        entry[slot] = Some(Arc::clone(&variants));
    }
    variants
}

fn spelling_variants(word: &str) -> Vec<Phones> {
    // ASR 偶尔保留拼音式辅音写法；只在一次可解释的辅音改写能命中字典时解析。
    let mut result = Vec::new();
    for (heard, spelling) in [("w", "v"), ("f", "th"), ("d", "th")] {
        if let Some(tail) = word.strip_prefix(heard) {
            result.extend(lookup(&format!("{spelling}{tail}")));
        }
    }
    for heard in ["f", "d"] {
        if let Some(prefix) = word.strip_suffix(heard) {
            result.extend(lookup(&format!("{prefix}th")));
        }
    }
    result
}

pub fn ipa_variants(word: &str) -> Option<Vec<String>> {
    // 撇号和连字符可能改变整个单词的读音，不能删除后误查另一个词。
    let normalized = word
        .trim_matches(|ch: char| {
            ch.is_whitespace()
                || (ch.is_ascii_punctuation() && !matches!(ch, '\'' | '-'))
                || matches!(
                    ch,
                    '“' | '”' | '。' | '，' | '！' | '？' | '；' | '：' | '（' | '）'
                )
        })
        .replace('’', "'")
        .to_ascii_lowercase();
    if normalized.is_empty()
        || !normalized
            .chars()
            .all(|ch| ch.is_ascii_alphabetic() || matches!(ch, '\'' | '-'))
    {
        return None;
    }
    let variants = pronunciations(&normalized, false);
    let output: Vec<_> = variants
        .iter()
        .filter_map(|phones| render_ipa(phones))
        .collect();
    (!output.is_empty()).then_some(output)
}

fn parse_phone(phone: &'static str) -> Phone {
    let (symbol, stress) = match phone.as_bytes().last().copied() {
        Some(b'0'..=b'2') => (
            &phone[..phone.len() - 1],
            phone.as_bytes()[phone.len() - 1] - b'0',
        ),
        _ => (phone, 0),
    };
    Phone { symbol, stress }
}

fn render_ipa(phones: &[Phone]) -> Option<String> {
    let mut ipa = String::new();
    for phone in phones {
        let symbol = ipa_symbol(phone.symbol, phone.stress)?;
        if is_vowel(phone.symbol) {
            match phone.stress {
                1 => ipa.push('ˈ'),
                2 => ipa.push('ˌ'),
                _ => {}
            }
        }
        ipa.push_str(symbol);
    }
    Some(format!("/{ipa}/"))
}

fn ipa_symbol(symbol: &str, stress: u8) -> Option<&'static str> {
    Some(match symbol {
        "AA" => "ɑː",
        "AE" => "æ",
        "AH" if stress == 0 => "ə",
        "AH" => "ʌ",
        "AO" => "ɔː",
        "AW" => "aʊ",
        "AY" => "aɪ",
        "B" => "b",
        "CH" => "tʃ",
        "D" => "d",
        "DH" => "ð",
        "EH" => "ɛ",
        "ER" if stress == 0 => "ɚ",
        "ER" => "ɝ",
        "EY" => "eɪ",
        "F" => "f",
        "G" => "ɡ",
        "HH" => "h",
        "IH" => "ɪ",
        "IY" => "iː",
        "JH" => "dʒ",
        "K" => "k",
        "L" => "l",
        "M" => "m",
        "N" => "n",
        "NG" => "ŋ",
        "OW" => "oʊ",
        "OY" => "ɔɪ",
        "P" => "p",
        "R" => "ɹ",
        "S" => "s",
        "SH" => "ʃ",
        "T" => "t",
        "TH" => "θ",
        "UH" => "ʊ",
        "UW" => "uː",
        "V" => "v",
        "W" => "w",
        "Y" => "j",
        "Z" => "z",
        "ZH" => "ʒ",
        "AX" => "ə",
        "AXR" => "ɚ",
        "DX" | "NX" => "ɾ",
        "EL" => "l̩",
        "EM" => "m̩",
        "EN" => "n̩",
        "IX" => "ɨ",
        "Q" => "ʔ",
        "UX" => "ʉ",
        _ => return None,
    })
}

fn is_vowel(symbol: &str) -> bool {
    matches!(
        symbol,
        "AA" | "AE"
            | "AH"
            | "AO"
            | "AW"
            | "AY"
            | "EH"
            | "ER"
            | "EY"
            | "IH"
            | "IY"
            | "OW"
            | "OY"
            | "UH"
            | "UW"
            | "AX"
            | "AXR"
            | "IX"
            | "UX"
    )
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{
        CACHE_WORD_LIMIT, PronunciationCache, Variants, WORDS, cache_variants, entry_word,
        ipa_variants, lookup, pronunciations,
    };

    #[test]
    fn ipa_lookup_reuses_cmu_lexicon_and_preserves_stress() {
        let hello = ipa_variants("HELLO!").unwrap();
        assert_eq!(hello, ["/həlˈoʊ/", "/hɛlˈoʊ/"]);
        assert!(ipa_variants("not-in-lexicon").is_none());
    }

    #[test]
    fn full_pronunciation_index_stays_below_one_megabyte() {
        assert!(WORDS.len() > 100_000, "仍使用完整发音表");
        assert!(std::mem::size_of_val(WORDS.as_ref()) < 1024 * 1024);
    }

    #[test]
    fn ipa_preserves_contractions_and_hyphenated_words() {
        assert_eq!(ipa_variants("don't").unwrap(), ["/dˈoʊnt/", "/dˈoʊn/"]);
        assert_eq!(ipa_variants("DON’T!"), ipa_variants("don't"));
        assert_eq!(ipa_variants("we'll").unwrap(), ["/wˈiːl/", "/wˈɪl/"]);
        assert_ne!(ipa_variants("we'll"), ipa_variants("well"));
        assert_eq!(ipa_variants("co-op").unwrap(), ["/kˈoʊˌɑːp/"]);
        assert_ne!(ipa_variants("co-op"), ipa_variants("coop"));
    }

    #[test]
    fn ipa_does_not_join_words_or_strip_internal_invalid_characters() {
        for word in [
            "co op", "he11o", "he/llo", "éclair", "中hello", "hello中", "...",
        ] {
            assert!(ipa_variants(word).is_none(), "{word}");
        }
    }

    #[test]
    fn sorted_index_keeps_source_order_regression_words_searchable() {
        assert!(
            WORDS
                .windows(2)
                .all(|pair| entry_word(pair[0]) <= entry_word(pair[1]))
        );
        for word in ["sepulveda", "sepultura", "stilton", "stilted"] {
            assert!(!lookup(word).is_empty(), "{word}");
        }
    }

    #[test]
    fn pronunciation_cache_reuses_exact_variants_without_deep_copy() {
        let mut cache = PronunciationCache::default();
        let first: Variants = Arc::from(lookup("hello"));
        cache_variants(&mut cache, "hello", false, true, Arc::clone(&first));
        let second = cache_variants(&mut cache, "hello", true, true, Arc::from(lookup("hello")));
        assert!(Arc::ptr_eq(&first, &second));
        assert!(Arc::ptr_eq(
            cache.0["hello"][0].as_ref().unwrap(),
            cache.0["hello"][1].as_ref().unwrap(),
        ));
    }

    #[test]
    fn cache_separates_spelling_fallback_from_exact_missing_words() {
        assert!(pronunciations("woice", false).is_empty());
        assert_eq!(
            pronunciations("woice", true),
            pronunciations("voice", false)
        );
        assert!(pronunciations("woice", false).is_empty());
    }

    #[test]
    fn cache_retains_misses_but_bounds_distinct_words() {
        let mut cache = PronunciationCache::default();
        let missing: Variants = Arc::from(Vec::new());
        let first = cache_variants(&mut cache, "missing", false, false, Arc::clone(&missing));
        let repeated = cache_variants(&mut cache, "missing", false, false, Arc::from(Vec::new()));
        assert!(Arc::ptr_eq(&first, &repeated));
        assert!(cache.0["missing"][1].is_none());
        for index in 0..CACHE_WORD_LIMIT * 2 {
            cache_variants(
                &mut cache,
                &index.to_string(),
                false,
                false,
                Arc::clone(&missing),
            );
            assert!(cache.0.len() <= CACHE_WORD_LIMIT);
            assert_eq!(cache.0.len(), cache.1.len());
        }
        assert!(!cache.0.contains_key("missing"));
        assert!(
            cache
                .0
                .contains_key(&(CACHE_WORD_LIMIT * 2 - 1).to_string())
        );
    }
}
