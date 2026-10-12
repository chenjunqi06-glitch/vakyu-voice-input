//! 发给识别进程的 JSONL 命令；每条命令带会话代次。

use serde::Serialize;

#[derive(Serialize)]
pub(super) struct HelperCommand {
    pub command: &'static str,

    pub generation: u64,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<&'static str>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub practice_text: Option<String>,
}
