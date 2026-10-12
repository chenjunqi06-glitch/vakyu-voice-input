//! 语音会话的归属、浮窗位置与心跳；结果不会跨应用投递。

use std::time::Instant;

use qingjian_platform::protocol::{ScreenRect, SessionId};

pub(in crate::dispatch) struct VoiceSession {
    pub id: SessionId,

    pub held: bool,

    pub anchor: ScreenRect,

    pub last_poll: Instant,

    /// None 是旧版直接开麦协议；新桥以序号隔离每个按住周期。
    pub sequence: Option<u64>,

    /// 尚在等待 Server 长按阈值时有值，此时没有请求麦克风。
    pub pending_since: Option<Instant>,
}
