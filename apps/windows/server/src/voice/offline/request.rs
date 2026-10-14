//! 发给离线翻译子进程的一行 JSON。

use serde::Serialize;

#[derive(Serialize)]
pub(super) struct Request<'a> {
    pub command: &'static str,

    pub id: u64,

    pub text: &'a str,

    /// `en` 或 `zh`。
    pub target: &'static str,
}
