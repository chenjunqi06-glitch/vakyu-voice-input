//! 薄桥只报告按住意图，开始录音的阈值与生命周期由 Server 决定。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VoiceAction {
    Press,
    Poll,
    Release,
    Cancel,
}
