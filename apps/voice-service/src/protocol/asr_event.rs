//! 本机 ASR 的事件，业务事件必须带录音代次。

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum AsrEvent {
    Ready,
    Started {
        generation: u64,
    },
    Interim {
        generation: u64,
        text: String,
    },
    Final {
        generation: u64,
        text: String,
    },
    Level {
        generation: u64,
        level: f32,
    },
    Error {
        generation: Option<u64>,
        message: String,
    },
}
