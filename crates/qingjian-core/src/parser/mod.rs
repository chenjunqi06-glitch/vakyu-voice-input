//! 拼音切分：把无分隔的拼音串切成音节序列。
//!
//! 支持全拼、简拼（声母缩写 `kf` → `k f`）与两者混用（`kaif`、`kfa`），输入允许用 `'` 强制分隔（`xi'an`）。
//! 末尾允许一个未打完的音节前缀（`zho` → `zho…`）。每种切分里每个音节标记是否完整，
//! 查词时完整音节精确匹配、不完整音节按前缀匹配。双拼、模糊音后续在这里扩展，接口保持不变。

mod error;
mod segmentation;
mod syllable;
mod trie;

use std::cmp::Reverse;
use std::sync::Arc;

pub use error::ParseError;
pub use segmentation::{Segmentation, Syllable};
pub use syllable::{INITIALS, MAX_SYLLABLE_LEN, SYLLABLES};

use syllable::initial_lengths;
use trie::SYLLABLE_TRIE;

pub fn is_syllable(s: &str) -> bool {
    SYLLABLE_TRIE.contains(s)
}

/// `s` 是否为某个合法音节的**真**前缀（不包含它自己就是完整音节的情况）。
pub fn is_syllable_prefix(s: &str) -> bool {
    SYLLABLE_TRIE.is_proper_prefix(s)
}

/// `text` 能否切成每个音节都完整的拼音。只回答是否，不产生切分、不分配音节：
/// 纠错要对上千个变体逐个问这个问题，先用它过滤，剩下的几个再做真正的切分。只认小写字母。
pub fn is_fully_segmentable(text: &str) -> bool {
    let n = text.len();
    if n == 0 {
        return false;
    }
    let mut reachable = vec![false; n + 1];
    reachable[0] = true;
    for start in 0..n {
        if !reachable[start] {
            continue;
        }
        for len in SYLLABLE_TRIE.matches(&text[start..]).lengths() {
            reachable[start + len] = true;
        }
    }
    reachable[n]
}

/// 返回能被切成合法音节（允许末尾未完成音节）的最长前缀字节数。
///
/// 这是输入缓冲区末尾混入暂时无法切分的字母时的快速路径。调用方随后只需对这个
/// 前缀调用一次 [`segment`]，避免从整串开始逐个缩短前缀而反复做相同的动态规划。
pub fn longest_segmentable_prefix_len(text: &str) -> Option<usize> {
    if text.is_empty()
        || !text
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte == b'\'')
    {
        return None;
    }

    let n = text.len();
    let bytes = text.as_bytes();
    let mut reachable = vec![false; n + 1];
    reachable[0] = true;
    for start in 0..n {
        if !reachable[start] {
            continue;
        }
        if bytes[start] == b'\'' {
            reachable[start + 1] = true;
            continue;
        }
        let rest = &text[start..];
        for len in SYLLABLE_TRIE.matches(rest).lengths() {
            reachable[start + len] = true;
        }
        for len in initial_lengths(rest) {
            reachable[start + len] = true;
        }
    }

    let mut longest = reachable
        .iter()
        .rposition(|reachable| *reachable)
        .unwrap_or(0);

    // A proper prefix is only allowed at the end of a segmentation. The maximum syllable
    // length is small, so checking these suffixes keeps this extension linear in input size.
    for start in 0..n {
        if !reachable[start] || bytes[start] == b'\'' {
            continue;
        }
        let end_limit = (start + MAX_SYLLABLE_LEN).min(n);
        for end in (start + 1..=end_limit).rev() {
            if bytes[end - 1] == b'\'' {
                continue;
            }
            if is_syllable_prefix(&text[start..end]) {
                longest = longest.max(end);
                break;
            }
        }
    }

    (longest > 0).then_some(longest)
}

/// 切分上限。简拼让歧义切分数量指数增长，每个位置只保留这么多种最优切分。
const MAX_SEGMENTATIONS: usize = 8;

/// 切分。返回按「音节少、不完整音节少、前面的音节长」排序的切分，最多 [`MAX_SEGMENTATIONS`] 种。
pub fn segment(input: &str) -> Result<Vec<Segmentation>, ParseError> {
    let input = input.trim();
    if input.is_empty() {
        return Err(ParseError::Empty);
    }
    for (position, ch) in input.chars().enumerate() {
        if !(ch.is_ascii_lowercase() || ch == '\'') {
            return Err(ParseError::InvalidChar {
                position: position + 1,
                ch,
            });
        }
    }

    // 按 `'` 切成若干段，各段独立切分后做笛卡尔积；只有最后一段允许残缺音节。
    let chunks: Vec<&str> = input.split('\'').filter(|c| !c.is_empty()).collect();
    if chunks.is_empty() {
        return Err(ParseError::NoSegmentation);
    }
    let mut results: Vec<Segmentation> = vec![Segmentation {
        syllables: Vec::new(),
    }];
    for (index, chunk) in chunks.iter().enumerate() {
        let is_last = index + 1 == chunks.len();
        let options = segment_chunk(chunk, is_last);
        if options.is_empty() {
            return Err(ParseError::NoSegmentation);
        }
        let mut next = Vec::with_capacity(results.len() * options.len());
        for base in &results {
            for option in &options {
                let mut syllables = base.syllables.clone();
                syllables.extend(option.syllables.iter().cloned());
                next.push(Segmentation { syllables });
            }
        }
        // 保留每个 chunk 合并后的最佳前缀，避免多个撇号造成笛卡尔积指数增长。
        prune(&mut next);
        results = next;
    }
    prune(&mut results);
    Ok(results)
}

/// 排序并截断到 [`MAX_SEGMENTATIONS`]。
fn prune(segmentations: &mut Vec<Segmentation>) {
    segmentations.sort_by_cached_key(sort_key);
    segmentations.dedup();
    segmentations.truncate(MAX_SEGMENTATIONS);
}

/// 音节少的优先；同音节数时不完整音节少的优先；再同则前面的音节越长越优先（贪心的结果排最前）。
fn sort_key(segmentation: &Segmentation) -> (usize, usize, Vec<Reverse<usize>>) {
    (
        segmentation.syllables.len(),
        segmentation.incomplete_count(),
        segmentation
            .syllables
            .iter()
            .map(|s| Reverse(s.text.len()))
            .collect(),
    )
}

/// 一个切分前缀用共享前驱节点表示，避免每个新音节都复制此前的整段 `Vec<Syllable>`。
#[derive(Clone)]
struct Path<'a> {
    tail: Option<Arc<PathNode<'a>>>,
    syllable_count: usize,
    incomplete_count: usize,
}

struct PathNode<'a> {
    previous: Option<Arc<PathNode<'a>>>,
    text: &'a str,
    complete: bool,
}

/// 切分不含 `'` 的一段：按位置做动态规划，每个位置只保留最优的几种前缀切分。
/// `allow_partial` 为真时末尾允许留一个残缺音节。
fn segment_chunk(chunk: &str, allow_partial: bool) -> Vec<Segmentation> {
    let n = chunk.len();
    let mut best: Vec<Vec<Path<'_>>> = vec![Vec::new(); n + 1];
    best[0].push(Path {
        tail: None,
        syllable_count: 0,
        incomplete_count: 0,
    });
    for start in 0..n {
        if best[start].is_empty() {
            continue;
        }
        prune_paths(&mut best[start]);
        let rest = &chunk[start..];
        let matches = SYLLABLE_TRIE.matches(rest);
        // (长度, 是否完整音节)
        let mut tokens: Vec<(usize, bool)> = matches.lengths().map(|len| (len, true)).collect();
        for len in initial_lengths(rest) {
            if !tokens.contains(&(len, true)) {
                tokens.push((len, false));
            }
        }
        // 整个剩余部分作为未打完的音节（`zho` → zhong / zhou …）。它本身是完整音节或纯声母时已经在上面了。
        let rest_is_complete = matches.lengths().next_back() == Some(rest.len());
        if allow_partial
            && matches.whole_is_prefix
            && !rest_is_complete
            && !tokens.contains(&(rest.len(), false))
        {
            tokens.push((rest.len(), false));
        }
        // Clone only the small path handles (their syllable tails are shared via `Arc`).
        let bases = best[start].clone();
        for (len, complete) in tokens {
            let text = &rest[..len];
            for base in &bases {
                best[start + len].push(Path {
                    tail: Some(Arc::new(PathNode {
                        previous: base.tail.clone(),
                        text,
                        complete,
                    })),
                    syllable_count: base.syllable_count + 1,
                    incomplete_count: base.incomplete_count + usize::from(!complete),
                });
            }
        }
    }
    prune_paths(&mut best[n]);
    std::mem::take(&mut best[n])
        .iter()
        .map(path_to_segmentation)
        .collect()
}

/// 对路径按原 `sort_key` 排序；长度比较从前往后，避免为排序分配 `Vec`。
fn compare_paths(left: &Path<'_>, right: &Path<'_>) -> std::cmp::Ordering {
    left.syllable_count
        .cmp(&right.syllable_count)
        .then_with(|| left.incomplete_count.cmp(&right.incomplete_count))
        .then_with(|| compare_path_lengths(left.tail.as_deref(), right.tail.as_deref()))
}

fn compare_path_lengths(
    left: Option<&PathNode<'_>>,
    right: Option<&PathNode<'_>>,
) -> std::cmp::Ordering {
    match (left, right) {
        (None, None) => std::cmp::Ordering::Equal,
        (Some(left), Some(right)) => {
            compare_path_lengths(left.previous.as_deref(), right.previous.as_deref())
                .then_with(|| right.text.len().cmp(&left.text.len()))
        }
        (None, Some(_)) => std::cmp::Ordering::Less,
        (Some(_), None) => std::cmp::Ordering::Greater,
    }
}

fn paths_equal(left: &Path<'_>, right: &Path<'_>) -> bool {
    fn nodes_equal(left: Option<&PathNode<'_>>, right: Option<&PathNode<'_>>) -> bool {
        match (left, right) {
            (None, None) => true,
            (Some(left), Some(right)) => {
                left.text == right.text
                    && left.complete == right.complete
                    && nodes_equal(left.previous.as_deref(), right.previous.as_deref())
            }
            _ => false,
        }
    }

    left.syllable_count == right.syllable_count
        && left.incomplete_count == right.incomplete_count
        && nodes_equal(left.tail.as_deref(), right.tail.as_deref())
}

fn prune_paths(paths: &mut Vec<Path<'_>>) {
    paths.sort_by(compare_paths);
    let mut unique = Vec::with_capacity(paths.len().min(MAX_SEGMENTATIONS));
    for path in paths.drain(..) {
        if !unique.iter().any(|previous| paths_equal(previous, &path)) {
            unique.push(path);
        }
    }
    unique.truncate(MAX_SEGMENTATIONS);
    *paths = unique;
}

fn path_to_segmentation(path: &Path<'_>) -> Segmentation {
    let mut nodes = Vec::with_capacity(path.syllable_count);
    let mut node = path.tail.as_deref();
    while let Some(current) = node {
        nodes.push(current);
        node = current.previous.as_deref();
    }
    nodes.reverse();
    Segmentation {
        syllables: nodes
            .into_iter()
            .map(|node| Syllable {
                text: node.text.to_owned(),
                complete: node.complete,
            })
            .collect(),
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    fn joined(input: &str) -> Vec<String> {
        segment(input)
            .unwrap()
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    #[test]
    fn greedy_segmentation_comes_first() {
        assert_eq!(joined("kaifa")[0], "kai fa");
        assert_eq!(joined("zhongwen")[0], "zhong wen");
    }

    #[test]
    fn ambiguous_input_yields_multiple_segmentations() {
        let all = joined("xian");
        assert_eq!(all[0], "xian");
        assert!(all.contains(&"xi an".to_owned()));
    }

    #[test]
    fn apostrophe_forces_boundary() {
        let all = joined("xi'an");
        assert_eq!(all[0], "xi an");
        assert!(!all.iter().any(|s| s.starts_with("xian")));
    }

    #[test]
    fn trailing_partial_syllable_is_marked() {
        assert_eq!(joined("kaif")[0], "kai f…");
        assert_eq!(joined("zho")[0], "zho…");
    }

    #[test]
    fn initials_only_segment_as_abbreviations() {
        assert_eq!(joined("kf")[0], "k… f…");
        assert_eq!(joined("zhw")[0], "zh… w…");
        assert!(joined("zhw").contains(&"z… h… w…".to_owned()));
    }

    #[test]
    fn mixed_full_and_abbreviated_syllables() {
        assert_eq!(joined("kfa")[0], "k… fa");
        assert_eq!(joined("kaif")[0], "kai f…");
        assert_eq!(joined("srf")[0], "s… r… f…");
    }

    #[test]
    fn complete_segmentation_ranks_before_partial() {
        let all = segment("xia").unwrap();
        assert_eq!(all[0].syllables, [Syllable::complete("xia")]);
    }

    #[test]
    fn long_input_stays_bounded() {
        let all = segment("womenjintianxiawuqukaihuiba").unwrap();
        assert!(all.len() <= MAX_SEGMENTATIONS);
        assert_eq!(all[0].to_string(), "wo men jin tian xia wu qu kai hui ba");
    }

    #[test]
    fn very_long_input_keeps_order_and_bound() {
        let input = "womenjintianxiawuqukaihuiba".repeat(8);
        let all = segment(&input).unwrap();
        assert!(all.len() <= MAX_SEGMENTATIONS);
        assert_eq!(all[0].incomplete_count(), 0);
        assert_eq!(all[0].syllables.len(), 80);
    }

    #[test]
    fn very_long_input_keeps_ambiguous_paths() {
        let input = "xian".repeat(30);
        let all = segment(&input).unwrap();
        assert!(all.len() <= MAX_SEGMENTATIONS);
        assert!(all.iter().any(|seg| seg.to_string().contains("xi an")));
    }

    #[test]
    fn full_segmentability_agrees_with_segment() {
        for text in ["nihao", "zhongguo", "xian", "a", "kaifazhe"] {
            assert!(is_fully_segmentable(text), "{text}");
            assert_eq!(segment(text).unwrap()[0].incomplete_count(), 0, "{text}");
        }
        for text in ["", "nih", "kaif", "zhzh", "v", "nihoa"] {
            assert!(!is_fully_segmentable(text), "{text}");
        }
    }

    #[test]
    fn longest_prefix_is_found_without_reparsing_each_suffix() {
        assert_eq!(longest_segmentable_prefix_len("kaifv"), Some(4));
        assert_eq!(longest_segmentable_prefix_len("zhov"), Some(3));
        assert_eq!(longest_segmentable_prefix_len("v"), None);
        assert_eq!(longest_segmentable_prefix_len("ni'v"), Some(3));
    }

    #[test]
    fn rejects_invalid_input() {
        assert_eq!(segment("").unwrap_err(), ParseError::Empty);
        assert_eq!(
            segment("kai1").unwrap_err(),
            ParseError::InvalidChar {
                position: 4,
                ch: '1'
            }
        );
        assert_eq!(segment("v").unwrap_err(), ParseError::NoSegmentation);
        assert_eq!(segment("kaiv").unwrap_err(), ParseError::NoSegmentation);
    }
}
