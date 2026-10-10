//! 跟读英文按测量宽度折行，滚动窗口围绕当前未完成单词，不裁掉实际练习内容。

use std::ops::Range;

use super::run::WordRun;

const MAX_LINES: usize = 5;

pub(super) fn wrap_words(
    words: &[String],
    width: i32,
    measure: impl Fn(&str) -> i32,
) -> Vec<Vec<WordRun>> {
    wrap_sized_words(words, width, measure(" "), |_, text| measure(text))
}

pub(super) fn english_source_lines(
    text: &str,
    width: i32,
    measure: impl Fn(&str) -> i32,
) -> Vec<String> {
    let mut words: Vec<String> = text
        .split_whitespace()
        .rev()
        .take(48)
        .map(str::to_owned)
        .collect();
    words.reverse();
    let lines = wrap_words(&words, width, measure);
    lines
        .iter()
        .skip(lines.len().saturating_sub(3))
        .map(|line| {
            line.iter()
                .map(|run| run.text.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect()
}

pub(super) fn wrap_sized_words(
    words: &[String],
    width: i32,
    space: i32,
    measure: impl Fn(usize, &str) -> i32,
) -> Vec<Vec<WordRun>> {
    let width = width.max(1);
    let mut lines = Vec::new();
    let mut line = Vec::new();
    let mut used = 0;
    for (index, word) in words.iter().enumerate() {
        for text in split_word(word, width, &|text| measure(index, text)) {
            let size = measure(index, &text);
            if !line.is_empty() && used + space + size > width {
                lines.push(std::mem::take(&mut line));
                used = 0;
            }
            if !line.is_empty() {
                used += space;
            }
            line.push(WordRun {
                index,
                text,
                width: size,
            });
            used += size;
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

fn split_word(word: &str, width: i32, measure: &impl Fn(&str) -> i32) -> Vec<String> {
    if measure(word) <= width {
        return vec![word.to_owned()];
    }
    let mut pieces = Vec::new();
    let mut piece = String::new();
    for ch in word.chars() {
        piece.push(ch);
        if measure(&piece) > width && piece.chars().count() > 1 {
            piece.pop();
            pieces.push(std::mem::take(&mut piece));
            piece.push(ch);
        }
    }
    if !piece.is_empty() {
        pieces.push(piece);
    }
    pieces
}

pub(super) fn visible_range(lines: &[Vec<WordRun>], matched: usize) -> Range<usize> {
    let current = lines
        .iter()
        .position(|line| line.iter().any(|run| run.index >= matched))
        .unwrap_or_else(|| lines.len().saturating_sub(1));
    let start = current
        .saturating_sub(2)
        .min(lines.len().saturating_sub(MAX_LINES));
    start..(start + MAX_LINES).min(lines.len())
}

/// 仅预览最近的识别片段；保持单行并优先保留尾部，便于看清刚说的词。
pub(super) fn recognition_feedback(
    recognized: &str,
    recording: bool,
    learn_english: bool,
    width: i32,
    measure: impl Fn(&str) -> i32,
) -> String {
    if recognized.trim().is_empty() {
        return if recording {
            if learn_english {
                "请从下划线词继续读"
            } else {
                "Continue from the underlined word"
            }
            .to_owned()
        } else {
            String::new()
        };
    }
    let mut tail: Vec<char> = recognized.chars().rev().take(120).collect();
    tail.reverse();
    let fragment: String = tail.iter().collect();
    let one_line = fragment.split_whitespace().collect::<Vec<_>>().join(" ");
    let chars: Vec<char> = one_line.chars().collect();
    let truncated = recognized.chars().count() > tail.len();
    for start in 0..=chars.len() {
        let ellipsis = if truncated || start > 0 { "…" } else { "" };
        let remaining: String = chars[start..].iter().collect();
        let label = if learn_english {
            "刚听到："
        } else {
            "Heard: "
        };
        let fitted = format!("{label}{ellipsis}{remaining}");
        if measure(&fitted) <= width {
            return fitted;
        }
    }
    String::new()
}
