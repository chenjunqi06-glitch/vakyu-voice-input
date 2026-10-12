//! 顺序读音对齐；近音必须消耗识别词，弱读省略单独计预算，不能跳过句子内容。

use super::{
    assessment::PracticeAssessment, level::PracticeLevel, matcher::EnglishPractice,
    normalize::normalize, phonetic, progress::PracticeProgress, similarity, step::MatchStep,
    tolerance,
};

impl EnglishPractice {
    pub(super) fn evaluate_with_level(
        &self,
        transcript: &str,
        level: PracticeLevel,
    ) -> PracticeAssessment {
        let mut result = PracticeAssessment {
            progress: PracticeProgress {
                total_words: self.words.len(),
                ..PracticeProgress::default()
            },
            ..PracticeAssessment::default()
        };
        if !self.valid {
            return result;
        }
        let recognized = normalize(transcript);
        if let Some(replay) = self.complete_replay(&recognized, level) {
            return replay;
        }
        let mut cursor = 0;
        let mut recent: Vec<String> = Vec::new();
        let mut previous_exact = false;
        let mut index = 0;
        while index < self.units.len() {
            let step = self
                .match_next(index, &recognized, cursor, &recent, level)
                .or_else(|| self.recover_word(index, &recognized, cursor, &recent, level));
            if let Some(step) = step {
                // 同时记住真正接受的 ASR 写法；近音词重读不能卡在已完成的进度上。
                recent.extend(recognized[step.start..step.end].iter().cloned());
                cursor = step.end;
                result.tolerated_words += step.tolerated;
                previous_exact = step.tolerated == 0;
                index = step.next_word;
            } else if index > 0
                && index + 1 < self.units.len()
                && previous_exact
                && result.omitted_words < omission_budget(level, self.words.len())
                && self.can_omit(index, &recognized, cursor, &recent)
            {
                result.omitted_words += 1;
                result.tolerated_words += 1;
                previous_exact = false;
                index += 1;
            } else {
                break;
            }
            result.progress.matched_words = index;
        }
        // 已完整匹配目标句后，尾随插话不应撤销跟读完成状态。
        result.progress.complete = index == self.words.len();
        result
    }

    /// An interrupted attempt may leave unrelated words around a fresh reading
    /// of the whole sentence. Accept that replay only when the target itself is
    /// matched in order without skipping unrelated speech inside the sentence.
    fn complete_replay(
        &self,
        recognized: &[String],
        level: PracticeLevel,
    ) -> Option<PracticeAssessment> {
        for start in self.recovery_window_start(recognized.len())..recognized.len() {
            let mut cursor = start;
            let mut recent = Vec::new();
            let mut tolerated_words = 0;
            let mut index = 0;
            while index < self.units.len() {
                let Some(step) = self.match_next(index, recognized, cursor, &recent, level) else {
                    break;
                };
                recent.extend(recognized[step.start..step.end].iter().cloned());
                cursor = step.end;
                tolerated_words += step.tolerated;
                index = step.next_word;
            }
            if index == self.units.len() {
                return Some(PracticeAssessment {
                    progress: PracticeProgress {
                        matched_words: self.words.len(),
                        total_words: self.words.len(),
                        complete: true,
                    },
                    tolerated_words,
                    omitted_words: 0,
                });
            }
        }
        None
    }

    pub(super) fn match_next(
        &self,
        index: usize,
        recognized: &[String],
        mut cursor: usize,
        recent: &[String],
        level: PracticeLevel,
    ) -> Option<MatchStep> {
        if let Some(name) = self.names.iter().find(|name| name.start == index) {
            while recognized
                .get(cursor)
                .is_some_and(|word| tolerance::filler(word))
            {
                cursor += 1;
            }
            return name
                .matches(recognized, cursor, level, self.match_config.name_similarity)
                .map(|(end, similar)| MatchStep {
                    start: cursor,
                    end,
                    next_word: name.end,
                    tolerated: if similar { name.end - name.start } else { 0 },
                });
        }
        match_word(
            &self.units[index],
            recognized,
            cursor,
            recent,
            level,
            self.match_config
                .word_similarity
                .saturating_add(match level {
                    PracticeLevel::Newcomer => 0,
                    PracticeLevel::Intermediate => 5,
                    PracticeLevel::Challenge => 15,
                }),
        )
        .map(|(end, similar)| MatchStep {
            start: cursor,
            end,
            next_word: index + 1,
            tolerated: usize::from(similar),
        })
    }

    fn can_omit(
        &self,
        index: usize,
        recognized: &[String],
        cursor: usize,
        recent: &[String],
    ) -> bool {
        let expected = &self.units[index];
        let previous = &self.units[index - 1];
        let next = &self.units[index + 1];
        expected.len() == 1
            && tolerance::weak(&expected[0])
            && previous.iter().all(|word| tolerance::content(word))
            && next.iter().all(|word| tolerance::content(word))
            && match_word(
                next,
                recognized,
                cursor,
                recent,
                PracticeLevel::Challenge,
                100,
            )
            .is_some_and(|(_, similar)| !similar)
    }
}

fn omission_budget(level: PracticeLevel, words: usize) -> usize {
    match level {
        PracticeLevel::Newcomer if words >= 5 => (words / 12).max(1),
        PracticeLevel::Intermediate => words / 30,
        _ => 0,
    }
}

fn match_word(
    expected: &[String],
    recognized: &[String],
    mut cursor: usize,
    recent: &[String],
    level: PracticeLevel,
    similarity_threshold: u8,
) -> Option<(usize, bool)> {
    let mut similar = false;
    for (index, unit) in expected.iter().enumerate() {
        loop {
            let heard = recognized.get(cursor)?;
            if heard == unit {
                cursor += 1;
                break;
            }
            let matched = (1..=3)
                .filter_map(|count| {
                    recognized
                        .get(cursor..cursor + count)
                        .map(|words| (count, words))
                })
                .find(|(_, words)| phonetic::similar(unit, words, level));
            if let Some((count, _)) = matched {
                cursor += count;
                similar = true;
                break;
            }
            if !tolerance::protected(unit)
                && !tolerance::protected(heard)
                && similarity::word_matches(unit, heard, similarity_threshold)
            {
                cursor += 1;
                similar = true;
                break;
            }
            let previous_units = &expected[..index];
            let repeated = recent.contains(heard) || previous_units.contains(heard);
            let numeric_and = heard == "and"
                && tolerance::numeric(unit)
                && (previous_units
                    .last()
                    .is_some_and(|word| tolerance::numeric(word))
                    || recent.last().is_some_and(|word| tolerance::numeric(word)));
            if !tolerance::filler(heard)
                && !tolerance::foreign_script_noise(heard)
                && !repeated
                && !numeric_and
            {
                return None;
            }
            cursor += 1;
        }
    }
    Some((cursor, similar))
}
