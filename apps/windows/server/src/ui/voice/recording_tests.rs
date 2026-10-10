//! 三种录音入口的帧缓存回归和生产渲染导出，不访问麦克风或网络。

use std::sync::Arc;
use std::time::Instant;

use windows::Win32::Foundation::RECT;
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};

use super::frame::VoiceFrame;
use super::{GLOBAL_HELD, VoiceWindow};
use crate::dispatch::RecordingMode;

const MODES: [(RecordingMode, &str, &str); 3] = [
    (
        RecordingMode::Dictation,
        "dictation",
        "我今天要去吃饭，明天下午三点去拿快递。",
    ),
    (
        RecordingMode::EnglishPractice,
        "english-practice",
        "我今天要去吃饭，明天下午三点去拿快递。",
    ),
    (
        RecordingMode::ChinesePractice,
        "chinese-practice",
        "I am going out for a meal today.",
    ),
];

fn assert_cached_mode(window: &VoiceWindow, expected: RecordingMode) {
    let frame = window.frame.borrow();
    let Some(VoiceFrame::Bubble { mode, .. }) = frame.as_ref() else {
        panic!("录音音量重画必须保留当前帧");
    };
    assert_eq!(*mode, expected);
}

#[test]
fn level_redraw_preserves_recording_mode_and_hide_clears_it() {
    let previous = GLOBAL_HELD.get();
    let window = VoiceWindow::new(Arc::new(|_| {})).expect("创建录音模式测试窗口");
    let anchor = RECT::default();
    for (mode, _, text) in MODES {
        window.set_held(true);
        window.show(text.to_owned(), anchor, "正在聆听", true, mode);
        for level in [25, 75, 0] {
            window.set_held(true);
            window.last_level_render.set(None);
            window.set_level(level);
            assert_cached_mode(&window, mode);
        }
        window.hide();
        assert!(window.frame.borrow().is_none());
        assert!(window.meter.borrow().quiet());
    }
    GLOBAL_HELD.set(previous);
}

#[test]
fn recording_mode_change_clears_previous_mode_audio_peaks() {
    let window = VoiceWindow::new(Arc::new(|_| {})).expect("创建录音模式缓存测试窗口");
    for (mode, _, text) in MODES {
        window.cache_frame(
            VoiceFrame::Bubble {
                text: text.to_owned(),
                anchor: RECT::default(),
                status: "正在聆听".to_owned(),
                mode,
                release_hint: mode.release_hint(),
            },
            true,
        );
        assert_cached_mode(&window, mode);
        assert!(window.meter.borrow().quiet());
        window.meter.borrow_mut().push(80);
    }
}

#[test]
fn recording_errors_preserve_mode_without_promising_text_delivery() {
    let previous = GLOBAL_HELD.get();
    let window = VoiceWindow::new(Arc::new(|_| {})).expect("创建录音错误测试窗口");
    for (mode, _, _) in MODES {
        window.set_held(true);
        window.show_error("请检查麦克风".to_owned(), RECT::default(), mode);
        assert_cached_mode(&window, mode);
        let frame = window.frame.borrow();
        let Some(VoiceFrame::Bubble { release_hint, .. }) = frame.as_ref() else {
            unreachable!();
        };
        assert_eq!(*release_hint, "松开结束，检查提示后重试");
        assert!(!window.recording.get());
    }
    window.hide();
    GLOBAL_HELD.set(previous);
}

#[test]
#[ignore = "指定 VAKYU_RECORDING_PREVIEW_DIR 导出三种录音模式 PNG，不访问麦克风"]
fn export_recording_modes_without_microphone() {
    let output = std::path::PathBuf::from(
        std::env::var_os("VAKYU_RECORDING_PREVIEW_DIR").expect("指定录音模式 PNG 目录"),
    );
    std::fs::create_dir_all(&output).unwrap();
    let _ = unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    let previous = GLOBAL_HELD.get();
    let window = VoiceWindow::new(Arc::new(|_| {})).expect("创建录音模式预览窗口");
    let anchor = RECT::default();
    for (mode, name, sample) in MODES {
        for (phase, text, status, recording) in [
            (
                "preparing",
                "准备就绪后再开始说话…",
                "正在准备麦克风",
                false,
            ),
            ("recording", sample, "正在聆听", true),
            (
                "error",
                "麦克风暂时不可用，请检查设备后重试",
                "语音暂不可用",
                false,
            ),
        ] {
            let path = output.join(format!("{name}-{phase}.png"));
            crate::ui::layered::export_next_frame(path.clone()).expect("设置录音预览输出路径");
            window.set_held(true);
            if phase == "error" {
                window.show_error(text.to_owned(), anchor, mode);
            } else {
                window.show(text.to_owned(), anchor, status, recording, mode);
            }
            assert_cached_mode(&window, mode);
            assert!(path.is_file(), "模式和状态预览应生成 PNG");
            if recording {
                let path = output.join(format!("{name}-level.png"));
                crate::ui::layered::export_next_frame(path.clone()).unwrap();
                // PNG 编码会暂停测试线程；模拟后台续约，不能把导出耗时当作用户松键。
                GLOBAL_HELD.set(Some((true, Instant::now())));
                window.set_held(true);
                window.last_level_render.set(None);
                window.set_level(70);
                assert_cached_mode(&window, mode);
                assert!(path.is_file(), "音量重画应保持同一模式");
            }
        }
        window.hide();
    }
    GLOBAL_HELD.set(previous);
}
