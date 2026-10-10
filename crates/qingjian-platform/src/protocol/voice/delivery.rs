//! 全局语音经宿主 TSF 编辑会话插入的事务；独立于 DLL 本地录音控制。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VoiceDeliveryCommand {
    Capture {
        request: u64,
    },
    Commit {
        request: u64,
        text: String,
        deadline_ms: u64,
    },
    Cancel {
        request: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VoiceDeliveryReport {
    Ready { request: u64 },
    Inserted { request: u64 },
    Rejected { request: u64 },
}
