//! 原生快捷键指定的单轮入口，录音途中不可隐式切换。

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Dictation,

    Practice,
}
