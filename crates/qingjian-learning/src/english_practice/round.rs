//! 单轮的固定等级和一次性奖励票据；不保存正文，也不可复制以重复领奖。

use super::{level::PracticeLevel, progress::PracticeProgress};

#[derive(Debug)]
pub struct PracticeRound {
    level: PracticeLevel,

    awarded: bool,
}

impl PracticeRound {
    pub(super) fn new(level: PracticeLevel) -> Self {
        Self {
            level,
            awarded: false,
        }
    }

    pub fn level(&self) -> PracticeLevel {
        self.level
    }

    pub(super) fn claim(&mut self, progress: PracticeProgress) -> Option<u64> {
        if self.awarded
            || !progress.complete
            || progress.total_words == 0
            || progress.matched_words != progress.total_words
        {
            return None;
        }
        self.awarded = true;
        Some(progress.total_words as u64)
    }
}
