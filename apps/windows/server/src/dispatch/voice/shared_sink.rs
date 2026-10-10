//! 可恢复的语音输出端：后台初始化失败时，把同一浮窗控制权交还给旧入口。

use std::sync::{Arc, Mutex};

use qingjian_platform::protocol::ScreenRect;

use super::{PracticeView, RecordingMode, VoiceSink};

#[derive(Clone)]
pub(super) struct SharedVoiceSink(Arc<Mutex<Box<dyn VoiceSink>>>);

impl SharedVoiceSink {
    pub(super) fn new(sink: Box<dyn VoiceSink>) -> Self {
        Self(Arc::new(Mutex::new(sink)))
    }
}

impl VoiceSink for SharedVoiceSink {
    fn set_voice_level(&self, level: u8) {
        if let Ok(sink) = self.0.lock() {
            sink.set_voice_level(level);
        }
    }

    fn set_voice_held(&self, held: bool) {
        if let Ok(sink) = self.0.lock() {
            sink.set_voice_held(held);
        }
    }

    fn prepare_voice(&self, rect: ScreenRect, mode: RecordingMode) {
        if let Ok(sink) = self.0.lock() {
            sink.prepare_voice(rect, mode);
        }
    }

    fn show_voice(&self, text: String, rect: ScreenRect, mode: RecordingMode) {
        if let Ok(sink) = self.0.lock() {
            sink.show_voice(text, rect, mode);
        }
    }

    fn show_practice(&self, view: PracticeView, rect: ScreenRect) {
        if let Ok(sink) = self.0.lock() {
            sink.show_practice(view, rect);
        }
    }

    fn voice_error(&self, message: String, rect: ScreenRect, mode: RecordingMode) {
        if let Ok(sink) = self.0.lock() {
            sink.voice_error(message, rect, mode);
        }
    }

    fn hide_voice(&self) {
        if let Ok(sink) = self.0.lock() {
            sink.hide_voice();
        }
    }
}
