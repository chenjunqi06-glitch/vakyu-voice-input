//! 跟读会话使用的语言匹配器；算法留在平台无关的 learning crate。

use qingjian_learning::{
    ChinesePractice, EnglishPractice, PracticeAssessment, PracticeLevel, PracticeMatchConfig,
};

pub(super) enum PracticeMatcher {
    English(EnglishPractice),

    Chinese(ChinesePractice),
}

impl PracticeMatcher {
    pub fn new(
        expected: &str,
        original: &str,
        level: PracticeLevel,
        config: PracticeMatchConfig,
        learn_english: bool,
    ) -> Self {
        if learn_english {
            Self::English(EnglishPractice::with_context_and_config(
                expected, original, level, config,
            ))
        } else {
            Self::Chinese(ChinesePractice::with_level_and_config(
                expected, level, config,
            ))
        }
    }

    pub fn words(&self) -> &[String] {
        match self {
            Self::English(matcher) => matcher.words(),
            Self::Chinese(matcher) => matcher.words(),
        }
    }

    pub fn is_valid(&self) -> bool {
        match self {
            Self::English(matcher) => matcher.is_valid(),
            Self::Chinese(matcher) => matcher.is_valid(),
        }
    }

    pub fn recognized_units(&self, transcript: &str) -> Vec<String> {
        match self {
            Self::English(_) => EnglishPractice::recognized_units(transcript),
            Self::Chinese(_) => ChinesePractice::recognized_units(transcript),
        }
    }

    pub fn evaluate_remaining(
        &self,
        transcript: &str,
        matched_words: usize,
        consumed_units: usize,
        previous_omissions: usize,
    ) -> (PracticeAssessment, usize) {
        match self {
            Self::English(matcher) => matcher.evaluate_remaining(
                transcript,
                matched_words,
                consumed_units,
                previous_omissions,
            ),
            Self::Chinese(matcher) => {
                let (progress, consumed) =
                    matcher.evaluate_remaining(transcript, matched_words, consumed_units);
                (
                    PracticeAssessment {
                        progress,
                        ..PracticeAssessment::default()
                    },
                    consumed,
                )
            }
        }
    }
}
