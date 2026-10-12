//! 语音浮窗的租约验证与不访问麦克风的桌面预览。

use std::sync::Arc;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{LPARAM, RECT, WPARAM};
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GWL_EXSTYLE, IsWindowVisible, MSG, PM_REMOVE, PeekMessageW, SW_SHOWNA,
    SetWindowLongPtrW, ShowWindow, TranslateMessage, WM_TIMER, WS_EX_APPWINDOW, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOPMOST,
};

use super::{
    GLOBAL_HELD, HELD_TIMER, PRACTICE_VISIBLE, VoiceWindow, global_held_at, window_held_at,
};
use crate::dispatch::PracticeView;

#[test]
fn global_lease_expires_and_release_never_falls_back_to_alt_state() {
    let now = Instant::now();
    assert_eq!(global_held_at(None, now), None);
    assert_eq!(global_held_at(Some((true, now)), now), Some(true));
    assert_eq!(global_held_at(Some((false, now)), now), Some(false));
    assert_eq!(
        global_held_at(Some((true, now)), now + Duration::from_millis(249)),
        Some(true)
    );
    assert_eq!(
        global_held_at(Some((true, now)), now + Duration::from_millis(250)),
        Some(false)
    );
}

#[test]
fn practice_visibility_outlasts_short_lease_but_explicit_release_wins() {
    let renewed = Instant::now();
    let later = renewed + Duration::from_secs(2);
    assert_eq!(
        window_held_at(Some((true, renewed)), later, false),
        Some(false)
    );
    assert_eq!(
        window_held_at(Some((true, renewed)), later, true),
        Some(true)
    );
    assert_eq!(
        window_held_at(Some((false, renewed)), later, true),
        Some(false)
    );
    assert_eq!(window_held_at(Some((true, later)), later, true), Some(true));
}

#[test]
fn current_practice_survives_expired_lease_timer_meter_and_renewal() {
    let previous = GLOBAL_HELD.get();
    let window = VoiceWindow::new(Arc::new(|_| {})).expect("创建跟读生命周期测试窗口");
    let stale = Instant::now() - Duration::from_secs(2);
    GLOBAL_HELD.set(Some((true, stale)));
    let practice = PracticeView {
        learn_english: true,
        pronunciations: Vec::new(),
        original: "你好".to_owned(),
        words: vec!["Hello.".to_owned()],
        matched_words: 0,
        level_label: "新手".to_owned(),
        level_progress: String::new(),
        recognized: String::new(),
        feedback: String::new(),
        actions_enabled: true,
        recording: false,
        word_card: None,
        status: "正在等待输入框".to_owned(),
    };
    window.show_practice(&practice, RECT::default());
    assert!(PRACTICE_VISIBLE.get());
    assert!(window.events.practice_view.borrow().is_some());
    assert!(unsafe { IsWindowVisible(window.hwnd) }.as_bool());
    unsafe { super::wndproc(window.hwnd, WM_TIMER, WPARAM(HELD_TIMER), LPARAM(0)) };
    assert!(unsafe { IsWindowVisible(window.hwnd) }.as_bool());
    window.set_level(0);
    window.set_held(true);
    assert!(window.frame.borrow().is_some());
    assert!(window.events.practice_view.borrow().is_some());
    assert!(unsafe { IsWindowVisible(window.hwnd) }.as_bool());

    window.set_held(false);
    assert!(!PRACTICE_VISIBLE.get());
    assert!(window.frame.borrow().is_none());
    assert!(window.events.practice_view.borrow().is_none());
    assert!(!unsafe { IsWindowVisible(window.hwnd) }.as_bool());
    window.show_practice(&practice, RECT::default());
    assert!(
        window.events.practice_view.borrow().is_none(),
        "迟到帧不能复活已关闭的轮次"
    );
    GLOBAL_HELD.set(previous);
}

/// 只画固定文字，不创建语音后端、不访问麦克风；供真机截图检查使用。
#[test]
#[ignore = "在桌面显示语音浮窗 30 秒，需人工检查"]
fn preview_voice_bubble_without_microphone() {
    let _ = unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    let window = VoiceWindow::new(Arc::new(|_| {})).expect("创建语音预览窗口");
    // 测试窗口可被桌面检查工具枚举，生产窗口仍是无任务栏入口的 TOOLWINDOW。
    unsafe {
        SetWindowLongPtrW(
            window.hwnd,
            GWL_EXSTYLE,
            (WS_EX_LAYERED | WS_EX_APPWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST).0 as isize,
        );
    }
    let anchor = RECT::default();
    let text = std::env::var("QINGJIAN_VOICE_PREVIEW_TEXT").unwrap_or_else(|_| {
        "今天下午我想先去吃饭，然后整理一下今天的工作。Vakyu 会把你说的话实时显示在这里。Hello, Vakyu!".to_owned()
    });
    window.sync_theme(anchor);
    window
        .render(&text, anchor, "正在聆听")
        .expect("绘制语音预览");
    // 不调 show：预览无需真实按住 Alt，也不启用松键自动隐藏定时器。
    let _ = unsafe { ShowWindow(window.hwnd, SW_SHOWNA) };
    let until = Instant::now() + Duration::from_secs(30);
    let mut message = MSG::default();
    while Instant::now() < until {
        // 分层窗口没有 WM_PAINT；重复贴图使 Windows Graphics Capture 能取到预览帧。
        window
            .render(&text, anchor, "正在聆听")
            .expect("更新语音预览");
        while unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE) }.as_bool() {
            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    window.hide();
}

/// 只画一词完成的跟读快照，不访问麦克风或翻译服务。
#[test]
#[ignore = "在桌面显示英语跟读浮窗 30 秒，需人工检查"]
fn preview_english_practice_first_word_without_microphone() {
    preview_practice(PracticeView {
        learn_english: true,
        pronunciations: Vec::new(),
        original: "我今天要去吃饭，明天下午三点去拿快递。".to_owned(),
        words: "I am going out for a meal today, and I will pick up my package at three tomorrow afternoon."
            .split_whitespace()
            .map(str::to_owned)
            .collect(),
        matched_words: 1,
        level_label: "新手 Lv.1".to_owned(),
        level_progress: "18 / 120 XP · 再积累 102 升级".to_owned(),
        recognized: "I".to_owned(),
        feedback: String::new(),
        actions_enabled: false,
        recording: true,
        word_card: None,
        status: "正在听你跟读 · 无需按住 Alt".to_owned(),
    });
}

/// 示范播放状态只用固定快照，预览不会播放声音或打开麦克风。
#[test]
#[ignore = "在桌面显示新手示范浮窗 30 秒，需人工检查"]
fn preview_english_practice_demo_without_microphone() {
    preview_practice(PracticeView {
        learn_english: true,
        pronunciations: Vec::new(),
        original: "我今天想去吃饭。".to_owned(),
        words: "I want to have a meal today."
            .split_whitespace()
            .map(str::to_owned)
            .collect(),
        matched_words: 0,
        level_label: "新手 Lv.1".to_owned(),
        level_progress: "0 / 120 XP · 再积累 120 升级".to_owned(),
        recognized: String::new(),
        feedback: String::new(),
        actions_enabled: false,
        recording: false,
        word_card: None,
        status: "示范朗读 · 先听一遍，随后自动开始跟读".to_owned(),
    });
}

/// 长英文中段必须继续显示当前词，不能一直停在段落开头。
#[test]
#[ignore = "在桌面显示长篇英语跟读浮窗 30 秒，需人工检查"]
fn preview_english_practice_middle_without_microphone() {
    let sentence = "Today I want to finish my work, have dinner with my friends, and spend some time practicing English. ";
    let words: Vec<_> = sentence
        .repeat(20)
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    let matched_words = words.len() / 2 + 5;
    preview_practice(PracticeView {
        learn_english: true,
        pronunciations: Vec::new(),
        original: "今天我想先完成工作，再和朋友一起吃晚饭，最后留一些时间练习英语。".repeat(10),
        words,
        matched_words,
        level_label: "进阶 Lv.2".to_owned(),
        level_progress: "248 / 500 XP · 再积累 252 升级".to_owned(),
        recognized: "Today I want to finish my work".to_owned(),
        feedback: String::new(),
        actions_enabled: false,
        recording: true,
        word_card: None,
        status: "正在听你跟读 · 无需按住 Alt".to_owned(),
    });
}

/// 识别偏差可直接看见；固定姓名样例不打开麦克风、不改变跟读匹配。
#[test]
#[ignore = "在桌面显示姓名识别反馈浮窗 30 秒，需人工检查"]
fn preview_english_practice_recognition_without_microphone() {
    preview_practice(PracticeView {
        learn_english: true,
        pronunciations: Vec::new(),
        original: "我叫陈俊奇。".to_owned(),
        words: "My name is Chen Junqi."
            .split_whitespace()
            .map(str::to_owned)
            .collect(),
        matched_words: 3,
        level_label: "新手 Lv.1".to_owned(),
        level_progress: "18 / 120 XP · 再积累 102 升级".to_owned(),
        recognized: "my name is chen jing".to_owned(),
        feedback: "识别到“my name is chen jing”；当前词是 Junqi".to_owned(),
        actions_enabled: true,
        recording: false,
        word_card: None,
        status: "请朗读英文 · 正在录音".to_owned(),
    });
}

fn preview_practice(practice: PracticeView) {
    let _ = unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    let window = VoiceWindow::new(Arc::new(|_| {})).expect("创建跟读预览窗口");
    unsafe {
        SetWindowLongPtrW(
            window.hwnd,
            GWL_EXSTYLE,
            (WS_EX_LAYERED | WS_EX_APPWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST).0 as isize,
        );
    }
    let anchor = RECT::default();
    window.sync_theme(anchor);
    let export_path =
        std::env::var_os("QINGJIAN_PRACTICE_PREVIEW_PNG").map(std::path::PathBuf::from);
    if let Some(path) = &export_path {
        crate::ui::layered::export_next_frame(path.clone()).expect("配置跟读预览 PNG 导出路径");
    }
    window
        .render_practice(&practice, anchor)
        .expect("绘制跟读预览");
    if let Some(path) = export_path {
        assert!(path.is_file(), "未生成指定的跟读预览 PNG");
        return;
    }
    let _ = unsafe { ShowWindow(window.hwnd, SW_SHOWNA) };
    let until = Instant::now() + Duration::from_secs(30);
    let mut message = MSG::default();
    while Instant::now() < until {
        window
            .render_practice(&practice, anchor)
            .expect("更新跟读预览");
        while unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE) }.as_bool() {
            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    window.hide();
}
