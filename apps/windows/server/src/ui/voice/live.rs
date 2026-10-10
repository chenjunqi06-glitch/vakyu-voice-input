//! 实测音量独立于转写刷新；只在录音阶段重画已经显示的内容。

use std::time::{Duration, Instant};

use super::VoiceWindow;
use super::frame::VoiceFrame;
use super::voice_held;

// 与后台真实音量更新统一为约 30 fps，避免多层节流把波形降到十余帧。
const LEVEL_FRAME_INTERVAL: Duration = Duration::from_millis(33);

impl VoiceWindow {
    pub(in crate::ui) fn set_level(&self, level: u8) {
        if !voice_held() {
            self.hide();
            return;
        }
        if !self.recording.get() {
            self.meter.borrow_mut().clear();
            self.level_dirty.set(false);
            return;
        }
        if self.meter.borrow_mut().push(level) {
            self.level_dirty.set(true);
        }
        let now = Instant::now();
        if !self.level_dirty.get()
            || (level != 0
                && self.last_level_render.get().is_some_and(|previous| {
                    now.saturating_duration_since(previous) < LEVEL_FRAME_INTERVAL
                }))
        {
            return;
        }
        // 跟读识别帧本身已经包含当前音量；紧接着再合成一次整窗只会重复占用 UI 线程。
        if matches!(
            self.frame.borrow().as_ref(),
            Some(VoiceFrame::Practice { .. })
        ) && self
            .last_practice_render
            .get()
            .is_some_and(|previous| now.saturating_duration_since(previous) < LEVEL_FRAME_INTERVAL)
        {
            return;
        }
        let result = {
            let frame = self.frame.borrow();
            match frame.as_ref() {
                Some(VoiceFrame::Bubble {
                    text,
                    anchor,
                    status,
                    mode,
                    release_hint,
                }) => self.render(text, *anchor, status, *mode, release_hint),
                Some(VoiceFrame::Practice { view, anchor }) => self.render_practice(view, *anchor),
                None => return,
            }
        };
        if let Err(error) = result {
            tracing::warn!(%error, "绘制语音音量失败");
            self.hide();
            return;
        }
        self.level_dirty.set(false);
        self.last_level_render.set(Some(now));
        if matches!(
            self.frame.borrow().as_ref(),
            Some(VoiceFrame::Practice { .. })
        ) {
            self.last_practice_render.set(Some(now));
        }
    }

    pub(super) fn cache_frame(&self, frame: VoiceFrame, recording: bool) {
        let changed_kind = self.frame.borrow().as_ref().is_some_and(|previous| {
            std::mem::discriminant(previous) != std::mem::discriminant(&frame)
                || matches!(
                    (previous, &frame),
                    (VoiceFrame::Bubble { mode: before, .. }, VoiceFrame::Bubble { mode: after, .. })
                        if before != after
                )
        });
        if !recording || self.recording.get() != recording || changed_kind {
            self.meter.borrow_mut().clear();
            self.level_dirty.set(false);
            self.last_level_render.set(None);
        }
        self.recording.set(recording);
        *self.frame.borrow_mut() = Some(frame);
    }
}
