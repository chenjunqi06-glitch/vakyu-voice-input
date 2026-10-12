//! 跟读轮次的放行门禁；临时结果只控制高亮，最终识别再次完整匹配才交付原文。

pub(in crate::voice::global) mod lifecycle;
pub(in crate::voice::global) use qingjian_practice::lookup;

#[cfg(test)]
mod tests;

pub(super) use qingjian_practice::{Phase, PracticeSession};
