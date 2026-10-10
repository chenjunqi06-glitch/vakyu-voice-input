//! 从尚未通过的显示词继续消费识别内容；已使用的 ASR 单元不能再次计分。

use super::{
    alignment::omission_budget, assessment::PracticeAssessment, matcher::EnglishPractice,
    normalize::normalize, progress::PracticeProgress,
};

impl EnglishPractice {
    /// 返回用于识别修订对齐的稳定单元；与普通跟读使用同一种归一化。
    pub fn recognized_units(transcript: &str) -> Vec<String> {
        normalize(transcript)
    }

    /// 返回累计显示词进度与本次实际消费到的识别单元位置。
    /// 调用者负责在识别器改写累计文本时映射 consumed_units，并保存已通过词。
    pub fn evaluate_remaining(
        &self,
        transcript: &str,
        matched_words: usize,
        consumed_units: usize,
        previous_omissions: usize,
    ) -> (PracticeAssessment, usize) {
        let recognized = normalize(transcript);
        let mut cursor = consumed_units.min(recognized.len());
        let mut index = matched_words.min(self.words.len());
        let mut recent = Vec::new();
        let mut previous_exact = false;
        let mut omitted = 0;
        let mut tolerated = 0;
        if self.valid {
            while index < self.units.len() {
                let Some(level) = self.level else {
                    let mut next_cursor = cursor;
                    for unit in &self.units[index] {
                        let Some(offset) = recognized[next_cursor..]
                            .iter()
                            .position(|word| word == unit)
                        else {
                            return (
                                PracticeAssessment {
                                    progress: self.remaining_progress(index),
                                    ..PracticeAssessment::default()
                                },
                                cursor,
                            );
                        };
                        next_cursor += offset + 1;
                    }
                    cursor = next_cursor;
                    index += 1;
                    continue;
                };
                // 每次只寻找当前尚未通过词；插话或错误的旧词不要求用户重读前缀。
                // 目标词仍逐个顺序消费，重复的目标词需要不同的识别位置。
                let step = (cursor..recognized.len())
                    .find_map(|start| self.match_next(index, &recognized, start, &recent, level));
                if let Some(step) = step {
                    recent.extend(recognized[step.start..step.end].iter().cloned());
                    cursor = step.end;
                    tolerated += step.tolerated;
                    previous_exact = step.tolerated == 0;
                    index = step.next_word;
                } else if index > matched_words
                    && index + 1 < self.units.len()
                    && previous_exact
                    && previous_omissions.saturating_add(omitted)
                        < omission_budget(level, self.words.len())
                    && self.can_omit(index, &recognized, cursor, &recent)
                {
                    omitted += 1;
                    tolerated += 1;
                    previous_exact = false;
                    index += 1;
                } else {
                    break;
                }
            }
        }
        (
            PracticeAssessment {
                progress: self.remaining_progress(index),
                tolerated_words: tolerated,
                omitted_words: omitted,
            },
            cursor,
        )
    }

    fn remaining_progress(&self, matched_words: usize) -> PracticeProgress {
        PracticeProgress {
            matched_words,
            total_words: self.words.len(),
            complete: self.valid && matched_words == self.words.len(),
        }
    }
}
