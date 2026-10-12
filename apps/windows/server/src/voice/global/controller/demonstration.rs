//! 示范朗读与跟读采集互斥；播放结束后留出短暂尾音时间，再复核目标并开麦。

use std::time::{Duration, Instant};

use crate::voice::global::demo::EnglishDemo;
use crate::voice::global::practice::Phase;

use super::Controller;

const PLAYBACK_TAIL: Duration = Duration::from_millis(400);

impl Controller {
    pub(super) fn start_demonstration(&mut self, english: &str) {
        if !self.config.practice_demo {
            self.start_practice_recording();
            return;
        }
        if !self.practice_target_current(true) {
            if let Some(session) = &mut self.practice {
                session.fail(
                    "原输入框已变化，原话尚未填入；请恢复原输入框后重试，或按 Esc 取消".to_owned(),
                );
            }
            self.show_practice();
            return;
        }
        let (Some(active), Some(session)) = (&self.active, &mut self.practice) else {
            return;
        };
        session.demonstrating();
        let sequence = active.sequence;
        let rate = session.demo_rate();
        let started = (|| {
            if self.demo.is_none() {
                self.demo = Some(EnglishDemo::new()?);
            }
            self.demo
                .as_mut()
                .expect("示范工作线程已创建")
                .speak_in_language(sequence, english, rate, self.config.target_language())
        })();
        if started.is_err() {
            tracing::warn!(sequence, "跟读示范无法启动，转为直接跟读");
            self.practice
                .as_mut()
                .expect("练习仍存在")
                .demo_finished(true);
            self.start_practice_recording();
        } else {
            self.sink.set_voice_held(true);
            self.show_practice();
            tracing::info!(sequence, rate, "跟读示范开始，麦克风保持关闭");
        }
    }

    pub(super) fn demonstration_tick(&mut self) {
        if !self
            .practice
            .as_ref()
            .is_some_and(|session| session.phase == Phase::Demonstrating)
        {
            return;
        }
        if let Some(result) = self.demo.as_mut().and_then(EnglishDemo::try_recv)
            && self
                .active
                .as_ref()
                .is_some_and(|active| active.sequence == result.request_id)
        {
            let failed = result.result.is_err();
            if failed {
                tracing::warn!(sequence = result.request_id, "跟读示范不可用，转为直接跟读");
            }
            // try_recv 只在播放结束/已停止时交付；不将示范音频当成用户跟读。
            self.demo_ready_at = Some((Instant::now() + PLAYBACK_TAIL, failed));
        }
        if let Some((ready, failed)) = self.demo_ready_at
            && Instant::now() >= ready
        {
            self.demo_ready_at = None;
            self.practice
                .as_mut()
                .expect("练习仍存在")
                .demo_finished(failed);
            self.start_practice_recording();
        }
    }
}
