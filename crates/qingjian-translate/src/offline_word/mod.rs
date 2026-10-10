//! 按需查询随包双向词义；不联网，也不建立常驻全量词典索引。

mod chinese;
mod english;
mod forms;
mod selection;
mod table;

pub use chinese::{chinese_pinyin, chinese_practice_readings, offline_chinese_meaning};
pub use english::offline_english_meaning;
pub use selection::chinese_practice_word;

#[cfg(test)]
mod tests;
