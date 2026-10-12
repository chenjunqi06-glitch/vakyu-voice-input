//! 语音专用编辑会话：开始时保存向后依附的空范围，结束时只在这个锚点插字。

mod anchor;
mod capture;
mod commit;
mod diagnostic;

pub(crate) use self::anchor::VoiceAnchor;
pub(crate) use self::capture::request_voice_anchor;
pub(crate) use self::commit::request_voice_commit;
