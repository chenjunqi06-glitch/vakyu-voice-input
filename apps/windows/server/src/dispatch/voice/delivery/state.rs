//! TSF 回投桥的锁内状态。

use qingjian_platform::protocol::SessionId;
use std::collections::HashMap;
use std::time::Instant;

#[derive(Default)]
pub(super) struct State {
    pub clients: HashMap<(u32, u32), (SessionId, Instant)>,
    pub next: u64,
    pub pending: Option<Pending>,
}

pub(super) struct Pending {
    pub request: u64,
    pub session: SessionId,
    pub process: u32,
    pub thread: u32,
    pub phase: Phase,
}

pub(super) enum Phase {
    Capturing,
    Ready,
    Commit { text: String, deadline_ms: u64 },
    Inserted,
    Rejected,

    Unconfirmed,
}
