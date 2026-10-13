//! 音量独立刷新时复用的可见内容，只留在 UI 线程内存中。

use windows::Win32::Foundation::RECT;

use crate::dispatch::{PracticeView, RecordingMode};

// 保留 UI 线程内的现有值语义，避免为每次界面刷新分配堆内存。
#[allow(clippy::large_enum_variant)]
pub(super) enum VoiceFrame {
    Bubble {
        text: String,

        anchor: RECT,

        status: String,

        mode: RecordingMode,

        release_hint: &'static str,
    },

    Practice {
        view: PracticeView,

        anchor: RECT,
    },
}
