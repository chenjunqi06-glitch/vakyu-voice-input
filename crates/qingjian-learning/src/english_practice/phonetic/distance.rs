//! 音素距离只给已知口音混淆低成本；不允许一般音素删除、词缀删除或任意替换。

use crate::english_practice::level::PracticeLevel;
use crate::english_practice::phonetic::lexicon::Phone;

pub(super) fn similar(expected: &[Phone], heard: &[Phone], level: PracticeLevel) -> bool {
    let expected: Vec<_> = expected.iter().map(|phone| phone.symbol).collect();
    let heard: Vec<_> = heard.iter().map(|phone| phone.symbol).collect();
    if expected == heard {
        return true;
    }
    if expected.is_empty() || level == PracticeLevel::Challenge {
        return false;
    }
    // 吞掉一个元音通常意味着漏了一个音节，不能按口音替代放行。
    let expected_vowels = expected.iter().filter(|phone| vowel(phone)).count();
    let heard_vowels = heard.iter().filter(|phone| vowel(phone)).count();
    if expected_vowels != heard_vowels || expected.len().abs_diff(heard.len()) > 1 {
        return false;
    }
    let limit = match level {
        PracticeLevel::Newcomer => (expected.len() * 12).clamp(25, 65),
        PracticeLevel::Intermediate => (expected.len() * 5).clamp(12, 30),
        PracticeLevel::Challenge => 0,
    };
    let width = heard.len() + 1;
    let mut costs = vec![1000usize; (expected.len() + 1) * width];
    costs[0] = 0;
    for i in 0..=expected.len() {
        for j in 0..=heard.len() {
            let current = costs[i * width + j];
            if current > limit {
                continue;
            }
            if i < expected.len() && j < heard.len() {
                let next = (i + 1) * width + j + 1;
                costs[next] = costs[next].min(current + substitution(expected[i], heard[j]));
            }
            // 美式非卷舌和卷舌变体只允许相同元音后的 R 差异，不泛化删尾辅音。
            if i < expected.len() && expected[i] == "R" && i > 0 && vowel(expected[i - 1]) {
                let next = (i + 1) * width + j;
                costs[next] = costs[next].min(current + 15);
            }
            if j < heard.len() && heard[j] == "R" && j > 0 && vowel(heard[j - 1]) {
                let next = i * width + j + 1;
                costs[next] = costs[next].min(current + 15);
            }
        }
    }
    costs[expected.len() * width + heard.len()] <= limit
}

fn substitution(left: &str, right: &str) -> usize {
    if left == right {
        return 0;
    }
    for group in [
        &["IH", "IY"][..],
        &["EH", "AE"],
        &["UH", "UW"],
        &["AH", "AA", "AO"],
        &["AH", "ER"],
    ] {
        if group.contains(&left) && group.contains(&right) {
            return 18;
        }
    }
    for pair in [
        ["TH", "S"],
        ["TH", "T"],
        ["TH", "F"],
        ["DH", "D"],
        ["DH", "Z"],
        ["R", "L"],
        ["V", "W"],
        ["V", "F"],
        ["Z", "S"],
        ["CH", "SH"],
        ["JH", "ZH"],
        ["B", "P"],
        ["D", "T"],
        ["G", "K"],
        ["N", "NG"],
    ] {
        if pair.contains(&left) && pair.contains(&right) {
            return 25;
        }
    }
    1000
}

fn vowel(phone: &str) -> bool {
    matches!(
        phone,
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
    )
}
