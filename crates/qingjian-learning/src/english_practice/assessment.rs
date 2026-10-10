//! 文字匹配进度与本轮使用的容错数；不代表声学发音评分。

use super::progress::PracticeProgress;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PracticeAssessment {
    pub progress: PracticeProgress,

    /// 音近或弱读省略的显示词总数。
    pub tolerated_words: usize,

    /// tolerated_words 中靠前后锚点确认的弱读省略词数。
    pub omitted_words: usize,
}
