//! 音量条只随样本变化，安静与阶段重置不保留旧波峰。

use super::VoiceMeter;

#[test]
fn silence_stays_flat_without_generated_motion() {
    let mut meter = VoiceMeter::default();
    for _ in 0..100 {
        assert!(!meter.push(0));
        assert!(meter.quiet());
        assert_eq!(meter.heights(), [2; 5]);
    }
}

#[test]
fn history_follows_measured_samples_and_clamps_out_of_range_values() {
    let mut meter = VoiceMeter::default();
    for level in [10, 25, 50, 75, 255] {
        assert!(meter.push(level));
    }
    assert_eq!(meter.heights(), [4, 7, 12, 17, 23]);
    assert!(!meter.quiet());
    assert!(meter.push(0));
    assert!(meter.quiet());
    assert_eq!(meter.heights(), [2; 5]);
}

#[test]
fn new_phase_starts_without_previous_recording_peaks() {
    let mut meter = VoiceMeter::default();
    meter.push(100);
    meter.clear();
    meter.push(20);
    assert_eq!(meter.heights(), [2, 2, 2, 2, 6]);
}

/// 读取实际 PCM 测得的 0–100 音量序列；不采集音频，不生成动画样本。
#[test]
#[ignore = "需要真实音量 JSON 序列与 PNG 输出目录，不访问麦克风"]
fn preview_voice_level_sequence_without_microphone() {
    use crate::dispatch::RecordingMode;
    use crate::ui::voice::VoiceWindow;
    use crate::ui::voice::frame::VoiceFrame;
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::HiDpi::{
        DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
    };

    let source = std::env::var_os("QINGJIAN_VOICE_LEVEL_FIXTURE").expect("指定真实音量 JSON 数组");
    let samples: Vec<u8> = serde_json::from_slice(&std::fs::read(source).unwrap()).unwrap();
    assert!(!samples.is_empty() && samples.iter().all(|level| *level <= 100));
    let peak = *samples.iter().max().unwrap();
    assert!(peak > 0, "输入序列必须包含实际说话采样");
    let quiet_index = samples
        .iter()
        .position(|level| *level == 0)
        .expect("输入序列应包含安静采样");
    let peak_index = samples.iter().position(|level| *level == peak).unwrap();
    let output = std::path::PathBuf::from(
        std::env::var_os("QINGJIAN_VOICE_LEVEL_PREVIEW_DIR").expect("指定预览 PNG 目录"),
    );
    std::fs::create_dir_all(&output).unwrap();
    let _ = unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    let window = VoiceWindow::new(std::sync::Arc::new(|_| {})).unwrap();
    let anchor = RECT::default();
    let text = "这段文字保持不变，音波随实际采样变化。";
    window.set_held(true);
    window.sync_theme(anchor);
    window.cache_frame(
        VoiceFrame::Bubble {
            text: text.to_owned(),
            anchor,
            status: "正在聆听".to_owned(),
            mode: RecordingMode::Dictation,
            release_hint: RecordingMode::Dictation.release_hint(),
        },
        true,
    );
    for (index, level) in samples
        .into_iter()
        .enumerate()
        .take(quiet_index.max(peak_index) + 1)
    {
        window.set_held(true);
        window.last_level_render.set(None);
        let export = if index == quiet_index {
            Some(output.join("voice-level-quiet.png"))
        } else if index == peak_index {
            Some(output.join("voice-level-speech.png"))
        } else {
            None
        };
        if let Some(path) = &export {
            crate::ui::layered::export_next_frame(path.clone()).unwrap();
        }
        window.set_level(level);
        if let Some(path) = export {
            // 安静初帧可能没有像素变化，不依赖下一次音量来导出这一帧。
            if level == 0 {
                window
                    .render(
                        text,
                        anchor,
                        "正在聆听",
                        RecordingMode::Dictation,
                        RecordingMode::Dictation.release_hint(),
                    )
                    .unwrap();
            }
            assert!(path.is_file(), "音量更新没有独立重画当前内容");
        }
    }
    window.set_held(false);
    assert!(window.meter.borrow().quiet());
    assert!(window.frame.borrow().is_none());
}
