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

    /// 与匹配器使用同一套汉字、专名和数字单元，对齐累计识别文本的修订。
    pub fn recognized_units(transcript: &str) -> Vec<String> {
        tokenize(transcript)
            .1
            .into_iter()
            .map(|unit| unit.normalized)
            .collect()
    }

    /// 只消费未使用的识别内容，返回本句累计进度和本次消费结束位置。
    pub fn evaluate_remaining(
        &self,
        transcript: &str,
        matched_words: usize,
        consumed_units: usize,
    ) -> (PracticeProgress, usize) {
        let (_, heard) = tokenize(transcript);
        let mut cursor = consumed_units.min(heard.len());
        let mut matched_words = matched_words.min(self.words.len());
        if self.valid {
            // 手动放行可能停在数字显示单元中间，按剩余显示文字重新分词。
            let remaining = self.words[matched_words..].join(" ");
            for expected in tokenize(&remaining).1 {
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
        (
            PracticeProgress {
                matched_words,
                total_words: self.words.len(),
                complete: self.valid && matched_words == self.words.len(),
            },
            cursor,
        )
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
