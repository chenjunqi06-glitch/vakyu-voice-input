//! 麦克风关闭确认的独立看门狗；取消输入会话后仍保留，直到收到同代次确认。

use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::event::HelperEvent;

pub(super) const CLOSE_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Default)]
pub(super) struct CloseWatchdog {
    /// 键为会话代次与是否取消；后来的停止确认不能清除仍未确认的取消。
    pending: HashMap<(u64, bool), Instant>,
}

impl CloseWatchdog {
    pub fn arm(&mut self, generation: u64, cancelled: bool, now: Instant) {
        self.pending
            .entry((generation, cancelled))
            .or_insert(now + CLOSE_TIMEOUT);
    }

    pub fn acknowledge(&mut self, event: &HelperEvent) -> bool {
        match event {
            HelperEvent::Stopped { generation } => {
                self.pending.remove(&(*generation, false));
                true
            }
            HelperEvent::Cancelled { generation } => {
                self.pending.remove(&(*generation, false));
                self.pending.remove(&(*generation, true));
                true
            }
            _ => false,
        }
    }

    pub fn check(&self, now: Instant) -> Result<(), String> {
        if self.pending.values().any(|deadline| now >= *deadline) {
            Err("麦克风关闭无响应，已终止语音组件，请稍后重新按住 Alt".to_owned())
        } else {
            Ok(())
        }
    }
}
