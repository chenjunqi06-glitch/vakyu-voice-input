//! 更新暂停是不可撤销屏障；迟到的配置命令不能重新开启录音或上屏。

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

pub(super) struct Permission {
    enabled: AtomicBool,

    suspended: AtomicBool,

    requested: AtomicU64,

    applied: AtomicU64,
}

impl Permission {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled: AtomicBool::new(enabled),
            suspended: AtomicBool::new(false),
            requested: AtomicU64::new(0),
            applied: AtomicU64::new(0),
        }
    }

    pub fn allowed(&self) -> bool {
        self.enabled.load(Ordering::Acquire)
            && !self.suspended.load(Ordering::Acquire)
            && self.requested.load(Ordering::Acquire) == self.applied.load(Ordering::Acquire)
    }

    pub fn invalidate(&self) -> u64 {
        self.requested
            .fetch_add(1, Ordering::AcqRel)
            .wrapping_add(1)
    }

    pub fn configure(&self, revision: u64, enabled: bool) {
        self.enabled.store(enabled, Ordering::Release);
        self.applied.store(revision, Ordering::Release);
    }

    pub fn suspend(&self) {
        self.suspended.store(true, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::Permission;

    #[test]
    fn late_configuration_cannot_undo_update_or_shutdown() {
        let permission = Permission::new(true);
        permission.suspend();
        permission.configure(0, true);
        assert!(!permission.allowed());
        permission.configure(0, false);
        permission.configure(0, true);
        assert!(!permission.allowed());
    }

    #[test]
    fn queued_disable_cannot_be_undone_by_earlier_enable() {
        let permission = Permission::new(true);
        let first = permission.invalidate();
        let second = permission.invalidate();
        permission.configure(first, true);
        assert!(!permission.allowed());
        permission.configure(second, false);
        assert!(!permission.allowed());
    }
}
