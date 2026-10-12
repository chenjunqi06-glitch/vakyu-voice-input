//! 识别进程的输出协议；无正文日志，旧代次只由会话管理器丢弃。

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub(super) enum HelperEvent {
    Ready,
    Started {
        generation: u64,
    },
    Level {
        generation: u64,
        level: u8,
    },
    Stopped {
        generation: u64,
    },
    Cancelled {
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
    Error {
        #[serde(default)]
        generation: Option<u64>,
        message: String,
    },
}

impl HelperEvent {
    pub fn generation(&self) -> Option<u64> {
        match self {
            Self::Ready => None,
            Self::Started { generation }
            | Self::Level { generation, .. }
            | Self::Stopped { generation }
            | Self::Cancelled { generation }
            | Self::Interim { generation, .. }
            | Self::Final { generation, .. } => Some(*generation),
            Self::Error { generation, .. } => *generation,
        }
    }
}
