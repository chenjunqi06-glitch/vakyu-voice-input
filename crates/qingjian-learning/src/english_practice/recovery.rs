//! 纠错与重读复用同一个读音比较器；补读仍须有连续的实际目标词，不能拼凑散词。

use super::{level::PracticeLevel, matcher::EnglishPractice, step::MatchStep, tolerance};

impl EnglishPractice {
    pub(super) fn recovery_window_start(&self, transcript_words: usize) -> usize {
        transcript_words.saturating_sub(self.units.len().saturating_add(32).max(64))
    }

    pub(super) fn recover_word(
        &self,
        index: usize,
        recognized: &[String],
        cursor: usize,
        recent: &[String],
        level: PracticeLevel,
    ) -> Option<MatchStep> {
        let left = &recent[recent.len().saturating_sub(2)..];
        let protected_target = self.units[index..]
            .iter()
            .any(|word| word.iter().any(|word| tolerance::protected(word)));
        let mut skipped_protected = recognized
            .get(cursor)
            .is_some_and(|word| tolerance::protected(word));
        let first_candidate = self
            .recovery_window_start(recognized.len())
            .max(cursor.saturating_add(1));
        for start in first_candidate..recognized.len() {
            let local = index > 0 && (protected_target || !skipped_protected);
            let restarted = !left.is_empty()
                && start >= left.len()
                && recognized[start - left.len()..start] == *left;
            let eligible = index == 0 || local || restarted || correction_marker(recognized, start);
            skipped_protected |= tolerance::protected(&recognized[start]);
            if !eligible {
                continue;
            }
            let Some(step) = self.match_next(index, recognized, start, &[], level) else {
                continue;
            };
            let mut next_index = step.next_word;
            let mut next_cursor = step.end;
            let limit = (next_index + 2).min(self.units.len());
            let mut context_complete = true;
            while next_index < limit {
                if let Some(next) = self.match_next(next_index, recognized, next_cursor, &[], level)
                {
                    next_index = next.next_word;
                    next_cursor = next.end;
                } else {
                    context_complete = false;
                    break;
                }
            }
            if !context_complete {
                continue;
            }
            return Some(step);
        }
        None
    }
}

fn correction_marker(recognized: &[String], start: usize) -> bool {
    matches!(
        start
            .checked_sub(1)
            .and_then(|index| recognized.get(index))
            .map(String::as_str),
        Some("sorry" | "actually" | "correction")
    ) || (start >= 2 && recognized[start - 2] == "i" && recognized[start - 1] == "mean")
}
