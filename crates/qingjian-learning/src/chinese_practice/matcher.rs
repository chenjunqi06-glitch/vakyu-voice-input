//! 中文显示单元保留标点，识别按顺序消费汉字或同音读音，不能跳过目标字。

use super::unit::{Unit, is_han, tokenize};
use crate::{PracticeLevel, PracticeMatchConfig, PracticeProgress};

#[derive(Clone, Debug)]
pub struct ChinesePractice {
    words: Vec<String>,

    units: Vec<Unit>,

    level: PracticeLevel,

    config: PracticeMatchConfig,

    valid: bool,
}

impl ChinesePractice {
    pub fn with_level_and_config(
        expected: &str,
        level: PracticeLevel,
        config: PracticeMatchConfig,
    ) -> Self {
        let (words, units) = tokenize(expected);
        let valid = !units.is_empty() && expected.chars().any(is_han);
        Self {
            words,
            units,
            level,
            config: config.normalized(),
            valid,
        }
    }

    /// 汉字各占一个显示单元，拉丁专名保留完整单词，标点附着前一个单元。
    pub fn words(&self) -> &[String] {
        &self.words
    }

    pub fn is_valid(&self) -> bool {
        self.valid
    }

    /// 传入整轮累计识别文字；回改可以回退，闲聊和重读不会阻止后续重试。
    /// 同音字通过仅代表 ASR 文本接近，不代表已评定声调或发音标准。
    pub fn evaluate(&self, transcript: &str) -> PracticeProgress {
        let (_, heard) = tokenize(transcript);
        let mut cursor = 0;
        let mut matched_words = 0;
        if self.valid {
            for expected in &self.units {
                let Some(offset) = heard[cursor..]
                    .iter()
                    .position(|unit| expected.matches(unit, self.level, self.config))
                else {
                    break;
                };
                cursor += offset + 1;
                matched_words += expected.display_words;
            }
        }
        PracticeProgress {
            matched_words,
            total_words: self.words.len(),
            complete: self.valid && matched_words == self.words.len(),
        }
    }
}
