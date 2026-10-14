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
    /// 划词翻译：宿主从系统「服务」菜单拿到的选中文字；与跟读互不影响。
    Translate {
        id: u64,
        text: String,
    },
    /// 回应 `translate_request`：`ok` 时 `text` 是译文，否则是失败原因；`fallback` 表示本机没有系统翻译。
    Translated {
        id: u64,
        ok: bool,
        text: String,
        #[serde(default)]
        fallback: bool,
    },
    Reload,
    Shutdown,
}
