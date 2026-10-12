//! 跟读会话使用的语言匹配器；算法留在平台无关的 learning crate。

use qingjian_learning::{
    ChinesePractice, EnglishPractice, PracticeLevel, PracticeMatchConfig, PracticeProgress,
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

    pub fn evaluate(&self, transcript: &str) -> PracticeProgress {
        match self {
            Self::English(matcher) => matcher.evaluate(transcript),
            Self::Chinese(matcher) => matcher.evaluate(transcript),
        }
    }
}
