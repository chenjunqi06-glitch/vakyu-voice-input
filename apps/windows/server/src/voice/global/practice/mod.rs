//! 跟读轮次的放行门禁；已通过进度保留，完成后仍需收尾确认才交付原文。

pub(in crate::voice::global) mod lifecycle;
pub(in crate::voice::global) use qingjian_practice::lookup;

#[cfg(test)]
mod tests;

pub(super) use qingjian_practice::{Phase, PracticeSession};
