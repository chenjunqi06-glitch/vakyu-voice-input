//! 界面可显式请求的跟读操作；点击空白处不是取消操作。

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PracticeAction {
    Retry,
    Skip,
    Accept,
    Lookup,
    CloseCard,
    SpeakWord,
}
