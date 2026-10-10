//! 英文目标句及逐词前缀匹配；额外语气词和重读可以忽略，目标词不能跳过。

use super::{
    assessment::PracticeAssessment,
    latin::supported_text,
    level::PracticeLevel,
    match_config::PracticeMatchConfig,
    names::{self, NameSpan},
    normalize::normalize,
    progress::PracticeProgress,
    similarity,
};

#[derive(Clone, Debug)]
pub struct EnglishPractice {
    pub(super) words: Vec<String>,

    pub(super) units: Vec<Vec<String>>,

    pub(super) valid: bool,

    pub(super) names: Vec<NameSpan>,

    pub(super) match_config: PracticeMatchConfig,

    pub(super) level: Option<PracticeLevel>,
}

impl EnglishPractice {
    pub fn new(expected: &str) -> Self {
        let mut words = Vec::new();
        let mut units = Vec::new();
        for word in expected.split_whitespace() {
            let normalized = normalize(word);
            if normalized.is_empty() {
                if let Some(previous) = words.last_mut() {
                    *previous += " ";
                    *previous += word;
                }
                continue;
            }
            words.push(word.to_owned());
            units.push(normalized);
        }
        // 翻译失败留下的中文，以及空白/纯标点，不能成为可放行的英语目标。
        let valid = !units.is_empty() && supported_text(expected);
        Self {
            words,
            units,
            valid,
            names: Vec::new(),
            match_config: PracticeMatchConfig::default(),
            level: None,
        }
    }

    /// 分级只作用于识别文字容错；它不能衡量真实发音质量。
    pub fn with_level(expected: &str, level: PracticeLevel) -> Self {
        Self::with_level_and_config(expected, level, PracticeMatchConfig::default())
    }

    pub fn with_level_and_config(
        expected: &str,
        level: PracticeLevel,
        match_config: PracticeMatchConfig,
    ) -> Self {
        // 在译句建立时预热，不把首次词典解析留到已经开麦的 Interim 处理中。
        super::phonetic::warm_up();
        Self {
            level: Some(level),
            match_config: match_config.normalized(),
            ..Self::new(expected)
        }
    }

    /// 原中文只用于确认译文中的拼音专名，不写入词库或练习进度文件。
    pub fn with_context(expected: &str, original: &str, level: PracticeLevel) -> Self {
        Self::with_context_and_config(expected, original, level, PracticeMatchConfig::default())
    }

    pub fn with_context_and_config(
        expected: &str,
        original: &str,
        level: PracticeLevel,
        match_config: PracticeMatchConfig,
    ) -> Self {
        let mut practice = Self::with_level_and_config(expected, level, match_config);
        practice.names = names::confirm(&practice.words, &practice.units, original);
        practice
    }

    /// ASR words are compared after lowercase and punctuation/spacing normalization.
    pub fn word_similarity(expected: &str, heard: &str) -> u8 {
        similarity::similarity_percent(expected, heard)
    }

    /// 显示词保留原有大小写和标点；一个缩写或数字只占一个显示词。
    pub fn words(&self) -> &[String] {
        &self.words
    }

    /// 不支持的文字不能被忽略；调用方可在开麦前拒绝无法跟读的目标。
    pub fn is_valid(&self) -> bool {
        self.valid
    }

    /// 输入必须是本轮从头开始的累计识别文本，不能只传新增长度。
    /// 临时识别修正时允许进度回退；上屏方仍应等待识别器的最终结果确认。
    pub fn evaluate(&self, transcript: &str) -> PracticeProgress {
        self.evaluate_details(transcript).progress
    }

    pub fn evaluate_details(&self, transcript: &str) -> PracticeAssessment {
        if let Some(level) = self.level {
            return self.evaluate_with_level(transcript, level);
        }
        PracticeAssessment {
            progress: self.evaluate_strict(transcript),
            ..PracticeAssessment::default()
        }
    }

    fn evaluate_strict(&self, transcript: &str) -> PracticeProgress {
        let recognized = normalize(transcript);
        let mut cursor = 0;
        let mut matched_words = 0;
        if self.valid {
            for expected in &self.units {
                let mut word_complete = true;
                for unit in expected {
                    let Some(offset) = recognized[cursor..].iter().position(|word| word == unit)
                    else {
                        word_complete = false;
                        break;
                    };
                    cursor += offset + 1;
                }
                if !word_complete {
                    break;
                }
                matched_words += 1;
            }
        }
        PracticeProgress {
            matched_words,
            total_words: self.words.len(),
            complete: self.valid && matched_words == self.words.len(),
        }
    }
}
