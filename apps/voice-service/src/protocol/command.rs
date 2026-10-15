//! 系统壳发送的命令；音频仅在当前代次内有效。

use qingjian_practice::PracticeAction;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum Command {
    Press {
        mode: Option<super::Mode>,
    },
    Release,
    Cancel,
    Audio {
        generation: u64,
        sample_rate: u32,
        pcm: String,
    },
    CaptureFailed {
        generation: u64,
        message: String,
    },
    SpeechFinished {
        id: u64,
        ok: bool,
    },
    Action {
        action: PracticeAction,
        index: Option<usize>,
    },
    CommitResult {
        id: u64,
        ok: bool,
        message: Option<String>,
    },
    Set {
        key: String,
        value: serde_json::Value,
    },
    /// 回应 `translate_request`：`ok` 时 `text` 是译文，否则是失败原因。
    Translated {
        id: u64,
        ok: bool,
        text: String,
    },
    Reload,
    Shutdown,
}
