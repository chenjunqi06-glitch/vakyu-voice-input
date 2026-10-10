//! 钩子线程公开的无正文原子状态。

use super::activity::ActivitySource;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU16, AtomicU64, Ordering};
use std::time::Instant;

pub(super) struct Shared {
    pub origin: Instant,

    pub alive: AtomicBool,

    pub heartbeat: AtomicU64,

    pub revision: AtomicU64,

    pub activity_source: AtomicU8,

    pub sequence: AtomicU64,

    pub key: AtomicU16,

    pub practice_active: AtomicBool,

    pub practice_escape: AtomicBool,
}

impl Shared {
    pub fn new() -> Self {
        Self {
            origin: Instant::now(),
            alive: AtomicBool::new(true),
            heartbeat: AtomicU64::new(0),
            revision: AtomicU64::new(0),
            activity_source: AtomicU8::new(ActivitySource::None as u8),
            sequence: AtomicU64::new(0),
            key: AtomicU16::new(0),
            practice_active: AtomicBool::new(false),
            practice_escape: AtomicBool::new(false),
        }
    }

    pub fn elapsed_ms(&self) -> u64 {
        self.origin.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
    }

    pub fn activity(&self, source: ActivitySource) -> u64 {
        self.activity_source.store(source as u8, Ordering::Release);
        self.revision.fetch_add(1, Ordering::AcqRel).wrapping_add(1)
    }
}
