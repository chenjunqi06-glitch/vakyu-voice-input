//! 一轮中文录入后的练习阶段，不把翻译失败或临时匹配当作完成。

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Translating,
    Demonstrating,
    Preparing,
    Reading,
    Checking,
    Failed,
    Complete,
}
