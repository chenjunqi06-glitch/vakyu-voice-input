//! 跟读匹配的显示词进度；完成只表示文字顺序吻合，不是发音评分。

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PracticeProgress {
    pub matched_words: usize,

    pub total_words: usize,

    pub complete: bool,
}
