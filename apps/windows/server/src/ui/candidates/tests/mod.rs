//! 显式运行的 Win32 候选窗基准：只创建自身无焦点窗口，不查询词库、不联网、不发送按键。

mod scenes;
mod timings;

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::Path;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use qingjian_platform::CandidateRenderer;
use tracing_subscriber::layer::SubscriberExt;
use windows::Win32::Foundation::{POINT, RECT};
use windows::Win32::UI::WindowsAndMessaging::{GetWindowRect, IsWindowVisible};

use crate::dispatch::RenderSettings;
use crate::ui::monitor;
use crate::ui::painter::{Painter, SharedPainter};

use self::scenes::frame;
use self::timings::{SlowStages, StageSamples, stats};
use super::CandidateWindow;

const SAMPLES: usize = 60;
const WARMUP: usize = 8;
#[test]
#[ignore = "显式 Win32 性能测试，会短暂显示自己的无焦点候选窗；请使用 --test-threads=1"]
fn candidate_window_end_to_end_benchmark() {
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let stages: StageSamples = Arc::new(Mutex::new(BTreeMap::new()));
    let subscriber = tracing_subscriber::registry().with(SlowStages(stages.clone()));
    let _subscriber = tracing::subscriber::set_default(subscriber);
    let painter: SharedPainter = Rc::new(RefCell::new(None));
    let startup = Instant::now();
    Painter::configure(
        &painter,
        &RenderSettings {
            renderer: CandidateRenderer::Miracle,
            font: String::new(),
        },
    );
    assert!(
        painter.borrow().is_some(),
        "基准必须使用真实 Miracle 渲染器，不能静默退回 GDI"
    );
    let startup_ms = startup.elapsed().as_secs_f64() * 1000.0;
    let window = CandidateWindow::new(painter).expect("创建测试候选窗");
    let work = monitor::work_area_near(POINT { x: 0, y: 0 });
    let anchor = RECT {
        left: work.left + 24,
        top: work.top + 24,
        right: work.left + 26,
        bottom: work.top + 48,
    };
    let mut cases = Vec::new();
    for (name, long, english) in [
        ("short_chinese_english_gloss", false, false),
        ("short_english_chinese_gloss", false, true),
        ("long_chinese_english_gloss", true, false),
        ("long_english_chinese_gloss", true, true),
    ] {
        let frames: Vec<_> = (0..36)
            .map(|revision| frame(long, english, revision))
            .collect();
        for revision in 0..WARMUP {
            window.set_content(&frames[revision % frames.len()]);
            window.show(anchor);
        }
        stages.lock().unwrap().clear();
        let mut totals = Vec::with_capacity(SAMPLES);
        let mut setup = Vec::with_capacity(SAMPLES);
        let mut shows = Vec::with_capacity(SAMPLES);
        for revision in 0..SAMPLES {
            let started = Instant::now();
            window.set_content(&frames[revision % frames.len()]);
            let show_started = Instant::now();
            window.show(anchor);
            let finished = Instant::now();
            setup.push(show_started.duration_since(started).as_secs_f64() * 1000.0);
            shows.push(finished.duration_since(show_started).as_secs_f64() * 1000.0);
            totals.push(finished.duration_since(started).as_secs_f64() * 1000.0);
            assert!(
                unsafe { IsWindowVisible(window.hwnd) }.as_bool(),
                "show 必须成功提交并显示自己的窗口"
            );
        }
        let mut rect = RECT::default();
        unsafe { GetWindowRect(window.hwnd, &mut rect) }.expect("读取自己候选窗尺寸");
        let stage_summary: BTreeMap<_, _> = stages
            .lock()
            .unwrap()
            .iter()
            .map(|(name, samples)| (name.clone(), stats(samples)))
            .collect();
        window.hide();
        assert!(!unsafe { IsWindowVisible(window.hwnd) }.as_bool());
        let total = stats(&totals);
        println!(
            "{profile} {name}: average={:.3}ms p95={:.3}ms max={:.3}ms",
            total["average_ms"].as_f64().unwrap(),
            total["p95_ms"].as_f64().unwrap(),
            total["max_ms"].as_f64().unwrap()
        );
        cases.push(serde_json::json!({
            "name": name, "sample_count": SAMPLES, "warmup_count": WARMUP,
            "width": rect.right - rect.left, "height": rect.bottom - rect.top, "dpi": window.dpi.get(),
            "total": total, "set_content": stats(&setup), "show": stats(&shows),
            "slow_stages_at_least_5ms": stage_summary,
        }));
    }
    drop(window);
    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target");
    std::fs::create_dir_all(&target).unwrap();
    let result = serde_json::json!({
        "profile": profile, "renderer": "Miracle", "painter_startup_ms": startup_ms, "cases": cases,
        "notes": "Full measured path: set_content + show, including real Win32 DIB allocation/conversion and UpdateLayeredWindow. Slow stages only capture production trace events >=5ms; absent events do not mean zero time. No input injection, network, dictionary or installed-process changes.",
    });
    std::fs::write(
        target.join(format!("perf-candidate-{profile}.json")),
        serde_json::to_vec_pretty(&result).unwrap(),
    )
    .unwrap();
}
