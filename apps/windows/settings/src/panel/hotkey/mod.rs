//! 设置页的英语跟读快捷键录入状态，后台确认保存后才更新界面中的有效键。

mod capture;
mod saved;
mod service;

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use qingjian_platform::PracticeShortcut;

pub(super) use capture::record;
pub(crate) use saved::Saved;
pub(super) use service::save;

#[derive(Default)]
pub(super) struct HotkeyEditor {
    pub generation: u64,

    pub recording: bool,

    pub saving: bool,

    pub candidate: Option<PracticeShortcut>,

    pub message: String,

    pub failed: bool,

    pub cancelled: Arc<AtomicBool>,
}

impl HotkeyEditor {
    pub fn begin(&mut self) -> (u64, Arc<AtomicBool>) {
        self.cancel();
        self.cancelled = Arc::new(AtomicBool::new(false));
        self.recording = true;
        self.candidate = None;
        self.message = "请按住新快捷键后全部松开；支持右 Ctrl + 右 Alt。Esc 取消。".to_owned();
        self.failed = false;
        (self.generation, Arc::clone(&self.cancelled))
    }

    pub fn cancel(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        self.generation = self.generation.wrapping_add(1);
        self.recording = false;
    }
}

impl Drop for HotkeyEditor {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
    }
}
