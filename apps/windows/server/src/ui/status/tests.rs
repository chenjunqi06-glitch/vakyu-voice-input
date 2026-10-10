//! 真正合成状态条，核对顶部字标不会触发按钮，并可导出两种渲染路径的预览。

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use qingjian_platform::{CandidateRenderer, ThemeMode};
use qingjian_render::Pixmap;

use crate::dispatch::{RenderSettings, StatusEvent, StatusView};
use crate::ui::candidates::theme::Theme;
use crate::ui::layered;
use crate::ui::painter::{Painter, SharedPainter};

use super::{StatusBar, logo};

#[test]
#[ignore = "显式 Win32 状态条验证，会短暂显示自身无焦点窗口；请使用 --test-threads=1"]
fn logo_header_does_not_toggle_controls() {
    let output = std::env::var_os("VAKYU_STATUS_PREVIEW").map(PathBuf::from);
    if let Some(output) = &output {
        std::fs::create_dir_all(output).unwrap();
    }
    for renderer in [CandidateRenderer::Miracle, CandidateRenderer::System] {
        let name = if renderer == CandidateRenderer::Miracle {
            "render"
        } else {
            "gdi"
        };
        let painter: SharedPainter = Rc::new(RefCell::new(None));
        Painter::configure(
            &painter,
            &RenderSettings {
                renderer,
                font: String::new(),
            },
        );
        assert_eq!(
            painter.borrow().is_some(),
            renderer == CandidateRenderer::Miracle
        );
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink = events.clone();
        let bar = StatusBar::new(
            Arc::new(move |event| sink.lock().unwrap().push(event)),
            painter,
        )
        .unwrap();
        *bar.data.borrow_mut() = Some(StatusView {
            english: false,
            zhuyin: false,
            scheme: None,
            full_width: true,
            theme: ThemeMode::Light,
            anchor: None,
        });
        for dpi in [96, 120, 144, 192] {
            bar.dpi.set(dpi);
            *bar.theme.borrow_mut() = Rc::new(Theme::new(dpi, false));
            let path = output
                .as_ref()
                .map(|dir| dir.join(format!("status-{name}-{dpi}.png")));
            if let Some(path) = &path {
                layered::export_next_frame(path.clone()).unwrap();
            }
            bar.render();
            let margin = bar.placement.margin.get();
            let (top, bottom) = bar.placement.controls_y.get();
            let edges: Vec<_> = bar
                .placement
                .cells
                .borrow()
                .iter()
                .map(|(edge, _)| *edge)
                .collect();
            assert_eq!(edges.len(), 3);
            assert!(edges.windows(2).all(|pair| pair[0] < pair[1]));
            let x = margin + edges[0] / 2;
            let y = margin + (top + bottom) / 2;
            events.lock().unwrap().clear();
            for (x, y) in [
                (x, margin - 1),
                (x, margin + top - 1),
                (margin - 1, y),
                (x, margin + bottom),
                (margin + edges[2], y),
            ] {
                bar.placement.on_click(x, y);
            }
            assert!(
                events.lock().unwrap().is_empty(),
                "logo/阴影不能误切换输入状态"
            );
            bar.placement.on_click(x, y);
            bar.placement
                .on_click(margin + (edges[0] + edges[1]) / 2, y);
            assert_eq!(
                *events.lock().unwrap(),
                [StatusEvent::ToggleMode, StatusEvent::TogglePunctuation]
            );
            logo::with_mark(dpi, |mark| {
                assert!(margin >= mark.height() as i32 / 2, "外露部分不能被位图裁剪");
                assert!(mark.height() as i32 / 2 < top, "logo不能盖住功能格");
                assert!(logo::left(dpi) + mark.width() as i32 <= edges[2]);
            })
            .expect("Vakyu完整字标必须解码成功");
            if let Some(path) = path {
                let frame = Pixmap::load_png(path).unwrap();
                assert!(
                    frame.pixels().iter().enumerate().any(|(index, p)| {
                        index / (frame.width() as usize) < margin as usize
                            && p.alpha() > 200
                            && p.red() > 180
                            && p.green() > 150
                            && p.blue() < 140
                    }),
                    "背景上缘外必须能看到黄色logo"
                );
            }
            bar.hide();
        }
    }
}
