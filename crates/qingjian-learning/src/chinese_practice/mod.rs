//! 中文跟读的逐字前缀匹配；复用英语练习进度与分级，不保存录音或正文。

mod latin;
mod matcher;
mod number;
mod unit;

pub use matcher::ChinesePractice;

#[cfg(test)]
mod tests;
