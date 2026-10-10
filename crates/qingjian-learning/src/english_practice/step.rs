//! 一步顺序对齐实际消耗的位置；专名可覆盖多个显示词，但不跳过任何音节。

pub(super) struct MatchStep {
    pub start: usize,

    pub end: usize,

    pub next_word: usize,

    pub tolerated: usize,
}
