//! 英文行内的一段文字；超宽单词可拆成多个共用同一单词序号的片段。

#[derive(Debug)]
pub(super) struct WordRun {
    pub index: usize,

    pub text: String,

    pub width: i32,
}
