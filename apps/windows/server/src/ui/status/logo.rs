//! 状态条左上角的完整 Vakyu 字标；半高外露，原图与最近 DPI 缩放结果只加载一次。

use std::cell::RefCell;
use std::sync::OnceLock;

use qingjian_render::Pixmap;
use tiny_skia::{FilterQuality, IntRect, PixmapPaint, Transform};

const LOGO_PNG: &[u8] = include_bytes!("../../../../../../assets/icon/windows/vakyu-logo.png");
const HEIGHT: f32 = 24.0;

/// 框内半个标志加两像素间隙，避免压住模式文字。
pub(super) const TOP_INSET: f32 = HEIGHT / 2.0 + 2.0;

static SOURCE: OnceLock<Option<Pixmap>> = OnceLock::new();

thread_local! {
    static SCALED: RefCell<Option<(u32, Pixmap)>> = const { RefCell::new(None) };
}

pub(super) fn inset(dpi: u32) -> i32 {
    (TOP_INSET * dpi.max(96) as f32 / 96.0).round() as i32
}

pub(super) fn left(dpi: u32) -> i32 {
    (6.0 * dpi.max(96) as f32 / 96.0).round() as i32
}

pub(super) fn with_mark<T>(dpi: u32, draw: impl FnOnce(&Pixmap) -> T) -> Option<T> {
    SCALED.with_borrow_mut(|cache| {
        if cache.as_ref().is_none_or(|(cached, _)| *cached != dpi) {
            *cache = scale_mark(dpi).map(|mark| (dpi, mark));
        }
        cache.as_ref().map(|(_, mark)| draw(mark))
    })
}

fn scale_mark(dpi: u32) -> Option<Pixmap> {
    let source = SOURCE.get_or_init(decode).as_ref()?;
    let height = (HEIGHT * dpi.max(96) as f32 / 96.0).round() as u32;
    let factor = height as f32 / source.height() as f32;
    let width = (source.width() as f32 * factor).ceil() as u32;
    let mut mark = Pixmap::new(width, height)?;
    mark.draw_pixmap(
        0,
        0,
        source.as_ref(),
        &PixmapPaint {
            quality: FilterQuality::Bicubic,
            ..PixmapPaint::default()
        },
        Transform::from_scale(factor, factor),
        None,
    );
    Some(mark)
}

fn decode() -> Option<Pixmap> {
    let pixmap = Pixmap::decode_png(LOGO_PNG).ok()?;
    let mut bounds = (pixmap.width(), pixmap.height(), 0, 0);
    for (index, pixel) in pixmap.pixels().iter().enumerate() {
        if pixel.alpha() <= 1 {
            continue;
        }
        let x = index as u32 % pixmap.width();
        let y = index as u32 / pixmap.width();
        bounds.0 = bounds.0.min(x);
        bounds.1 = bounds.1.min(y);
        bounds.2 = bounds.2.max(x + 1);
        bounds.3 = bounds.3.max(y + 1);
    }
    // 去掉素材周围的透明空白，让“半高外露”按实际可见字标计算。
    let rect = IntRect::from_ltrb(
        bounds.0 as i32,
        bounds.1 as i32,
        bounds.2 as i32,
        bounds.3 as i32,
    )?;
    pixmap.clone_rect(rect)
}
