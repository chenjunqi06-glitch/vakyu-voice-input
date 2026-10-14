//! 音量独立刷新时复用的可见内容，只留在 UI 线程内存中。

use windows::Win32::Foundation::RECT;

use crate::dispatch::{PracticeView, RecordingMode};

pub(super) enum VoiceFrame {
    Bubble {
        text: String,

        anchor: RECT,

        status: String,

        mode: RecordingMode,

        release_hint: &'static str,
    },

    Practice {
        view: Box<PracticeView>,

        anchor: RECT,
    },
}
