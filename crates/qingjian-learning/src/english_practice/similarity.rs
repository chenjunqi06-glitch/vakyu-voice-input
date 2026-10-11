//! Normalized edit-distance similarity used only as a guarded ASR fallback.

pub fn similarity_percent(expected: &str, heard: &str) -> u8 {
    let expected = normalize(expected);
    let heard = normalize(heard);
    normalized_similarity(&expected, &heard)
}

fn normalized_similarity(expected: &[char], heard: &[char]) -> u8 {
    if expected.is_empty() && heard.is_empty() {
        return 100;
    }
    let width = expected.len().max(heard.len());
    let distance = levenshtein(expected, heard);
    ((width.saturating_sub(distance) * 100) / width).min(100) as u8
}

pub(super) fn word_matches(expected: &str, heard: &str, threshold: u8) -> bool {
    let expected = normalize(expected);
    let heard = normalize(heard);
    if expected.len() < 4 || heard.len() < 4 {
        return false;
    }
    let similarity = normalized_similarity(&expected, &heard);
    similarity > threshold.min(100)
}

fn normalize(text: &str) -> Vec<char> {
    let mut normalized = Vec::new();
    for character in text.chars().flat_map(char::to_lowercase) {
        if character.is_alphanumeric() {
            normalized.push(character);
        } else if !normalized.is_empty() && normalized.last() != Some(&' ') {
            normalized.push(' ');
        }
    }
    if normalized.last() == Some(&' ') {
        normalized.pop();
    }
    normalized
}

fn levenshtein(left: &[char], right: &[char]) -> usize {
    let mut previous: Vec<usize> = (0..=right.len()).collect();
    let mut current = vec![0; right.len() + 1];
    for (left_index, left_char) in left.iter().enumerate() {
        current[0] = left_index + 1;
        for (right_index, right_char) in right.iter().enumerate() {
            current[right_index + 1] = (previous[right_index + 1] + 1)
                .min(current[right_index] + 1)
                .min(previous[right_index] + usize::from(left_char != right_char));
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[right.len()]
}

#[cfg(test)]
mod tests {
    use super::{similarity_percent, word_matches};

    #[test]
    fn normalizes_case_punctuation_and_spacing_before_comparing() {
        assert_eq!(similarity_percent("My, NAME!", "my name"), 100);
    }

    #[test]
    fn adjustable_threshold_accepts_small_asr_spelling_changes() {
        assert!(word_matches("beautiful", "beautifull", 80));
        assert!(!word_matches("beautiful", "beautifully", 85));
        assert!(!word_matches("fish", "wish", 75));
    }
}
