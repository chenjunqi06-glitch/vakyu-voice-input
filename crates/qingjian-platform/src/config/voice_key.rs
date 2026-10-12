//! Windows 语音按住键；选择单侧时不会占用另一侧 Alt。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoiceKey {
    #[default]
    Alt,
    LeftAlt,
    RightAlt,
}
