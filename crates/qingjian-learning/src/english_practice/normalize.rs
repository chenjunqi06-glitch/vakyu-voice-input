//! 识别文本和目标文本共用的保守归一化，不做模糊拼写或同义词匹配。

use super::{latin::fold_latin, number::expand_number};

pub(super) fn normalize(text: &str) -> Vec<String> {
    let characters: Vec<char> = text.chars().collect();
    let mut words = Vec::new();
    let mut current = String::new();
    for (index, &character) in characters.iter().enumerate() {
        let next_digit = characters.get(index + 1).is_some_and(char::is_ascii_digit);
        match character {
            '\'' | '’' | '‘' => current.push('\''),
            ',' if current.ends_with(|c: char| c.is_ascii_digit()) && next_digit => {}
            '.' if current.chars().all(|c| c.is_ascii_digit())
                && !current.is_empty()
                && next_digit =>
            {
                current.push('.');
            }
            '&' | '%' | '+' => {
                push_word(&mut words, &mut current);
                words.push(
                    match character {
                        '&' => "and",
                        '%' => "percent",
                        _ => "plus",
                    }
                    .to_owned(),
                );
            }
            '-' if current.is_empty() && next_digit => words.push("minus".to_owned()),
            '\u{0300}'..='\u{036f}' if current.ends_with(|c: char| c.is_ascii_alphabetic()) => {}
            character if character.is_alphanumeric() => {
                // 双语 ASR 常把英语与汉字姓名直接连写，保留两边内容但拆开文字边界。
                let is_latin = character.is_ascii() || fold_latin(character).is_some();
                if current
                    .chars()
                    .rev()
                    .find(|ch| ch.is_alphanumeric())
                    .is_some_and(|last| last.is_ascii() != is_latin)
                {
                    push_word(&mut words, &mut current);
                }
                if let Some(folded) = fold_latin(character) {
                    current.push_str(folded);
                } else {
                    current.extend(character.to_lowercase());
                }
            }
            _ => push_word(&mut words, &mut current),
        }
    }
    push_word(&mut words, &mut current);
    words
}

fn push_word(words: &mut Vec<String>, current: &mut String) {
    let raw = std::mem::take(current);
    let word = raw.trim_matches('\'');
    if word.is_empty() {
        return;
    }
    if expand_number(word, words) {
        return;
    }
    let expansion = match word {
        "can't" | "cannot" => Some("can not"),
        "won't" => Some("will not"),
        "shan't" => Some("shall not"),
        "i'm" => Some("i am"),
        "let's" => Some("let us"),
        "it's" => Some("it is"),
        "that's" => Some("that is"),
        "there's" => Some("there is"),
        "here's" => Some("here is"),
        "he's" => Some("he is"),
        "she's" => Some("she is"),
        "what's" => Some("what is"),
        "who's" => Some("who is"),
        "where's" => Some("where is"),
        "when's" => Some("when is"),
        "how's" => Some("how is"),
        _ => None,
    };
    if let Some(expansion) = expansion {
        words.extend(expansion.split_whitespace().map(str::to_owned));
        return;
    }
    for (suffix, expanded) in [
        ("n't", "not"),
        ("'re", "are"),
        ("'ve", "have"),
        ("'ll", "will"),
    ] {
        if let Some(base) = word.strip_suffix(suffix).filter(|base| !base.is_empty()) {
            words.push(base.to_owned());
            words.push(expanded.to_owned());
            return;
        }
    }
    words.push(word.to_owned());
}
