//! 一次按住的目标与交付状态；用户继续操作后，不保留可上屏的迟到结果。

use std::time::{Duration, Instant};

use super::target::LockedTarget;

pub(super) struct Active {
    pub sequence: u64,

    pub revision: u64,

    pub target: Option<LockedTarget>,

    pub error: Option<String>,

    pub held: bool,

    pub started: bool,

    pub final_text: Option<String>,

    pub since: Instant,

    pub pending_key_event: Option<Instant>,
}

impl Active {
    pub fn finish_timed_out(&self, now: Instant) -> bool {
        // 持续按住没有总时长截止；松键后给识别线程的 12 秒看门狗留出回包时间。
        !self.held && now.duration_since(self.since) >= Duration::from_secs(15)
    }
}

#[cfg(test)]
mod tests {
    use super::Active;
    use std::time::{Duration, Instant};

    #[test]
    fn long_hold_does_not_expire_but_release_still_has_a_watchdog() {
        let start = Instant::now();
        let mut active = Active {
            sequence: 1,
            revision: 1,
            target: None,
            error: None,
            held: true,
            started: true,
            final_text: None,
            since: start,
            pending_key_event: None,
        };
        for seconds in [299, 300, 301, 600, 3600] {
            assert!(!active.finish_timed_out(start + Duration::from_secs(seconds)));
        }
        active.held = false;
        active.since = start + Duration::from_secs(3600);
        assert!(!active.finish_timed_out(active.since + Duration::from_secs(14)));
        assert!(active.finish_timed_out(active.since + Duration::from_secs(15)));
    }
}
