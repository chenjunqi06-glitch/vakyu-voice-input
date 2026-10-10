//! 分层窗口合成：圆角背景 + 四周柔和阴影 + 一段 GDI 内容，合成进一张预乘 alpha 的 BGRA 位图，
//! `UpdateLayeredWindow` 一次贴上。候选窗口与状态条共用。位图在 [`Canvas`]，内容圆角矩形在 [`RoundRect`]。
//! Miracle 渲染器画好的整张位图（已含阴影）走 [`present`]，只做 RGBA → BGRA 再贴。

mod canvas;
mod round_rect;

#[cfg(test)]
mod preview_export;

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(in crate::ui) use preview_export::export_next_frame;

use qingjian_render::Pixmap;
use windows::Win32::Foundation::{COLORREF, E_INVALIDARG, HWND, POINT, RECT, SIZE};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BLENDFUNCTION, HDC, SetViewportOrgEx,
};
use windows::Win32::UI::WindowsAndMessaging::{ULW_ALPHA, UpdateLayeredWindow};
use windows::core::{Error, Result};

use self::canvas::Canvas;
use self::round_rect::RoundRect;

/// 内容四周留给阴影的宽度（逻辑像素）。
const SHADOW_MARGIN: i32 = 16;

/// 定向主阴影（光从上方来）的最浓 alpha：底部拿满、两侧减半、顶部为 0。
const KEY_MAX: f64 = 65.0;

/// 环境光晕的最浓 alpha，四边等浓。
const AMBIENT_MAX: f64 = 18.0;

/// 阴影留白的物理像素宽度（`dpi` 96 为 100%）。
pub(super) fn shadow_margin(dpi: u32) -> i32 {
    ((SHADOW_MARGIN * dpi as i32) / 96).max(1)
}

/// 一次合成的输入。
pub(super) struct Layered<'a> {
    /// 内容尺寸（不含阴影留白）。
    pub content: (i32, i32),

    /// 阴影留白（= [`shadow_margin`]）。
    pub margin: i32,

    /// 窗口左上角屏幕坐标（= 内容左上角 − 留白）。
    pub win_pos: (i32, i32),

    /// 窗口尺寸（= 内容 + 2·留白）。
    pub win_size: (i32, i32),

    /// 内容背景色。
    pub background: COLORREF,

    /// 右下角的渐变色；与起始色相同即可画纯色。
    pub background_end: COLORREF,

    /// 内容圆角半径。
    pub corner_radius: i32,

    /// 在内容坐标系里画内容；`client` 是 `{0, 0, w, h}`。
    pub paint: &'a dyn Fn(HDC, RECT),
}

/// 在合成位图坐标里叠加的小型透明标记。
pub(super) struct PixmapOverlay<'a> {
    pub pixmap: &'a Pixmap,

    pub x: i32,

    pub y: i32,
}

/// 合成一帧并贴到分层窗口上。
pub(super) fn composite(hwnd: HWND, layered: &Layered) -> Result<()> {
    composite_with_overlay(hwnd, layered, None)
}

/// 合成 GDI 内容并在最后叠加透明标记。
pub(super) fn composite_with_overlay(
    hwnd: HWND,
    layered: &Layered,
    overlay: Option<PixmapOverlay<'_>>,
) -> Result<()> {
    let (w, h) = layered.win_size;
    if w <= 0 || h <= 0 {
        return Err(Error::from(E_INVALIDARG));
    }
    let mut canvas = Canvas::new(w, h)?;
    let round = RoundRect::content(layered.content, layered.margin, layered.corner_radius);
    fill_shadow_and_background(
        canvas.pixels(),
        w,
        h,
        &round,
        (layered.background, layered.background_end),
        layered.margin,
    );

    // 视口原点挪到内容左上，内容闭包按自己的坐标画。
    let hdc = canvas.dc();
    unsafe {
        let _ = SetViewportOrgEx(hdc, layered.margin, layered.margin, None);
    }
    let client = RECT {
        left: 0,
        top: 0,
        right: layered.content.0,
        bottom: layered.content.1,
    };
    (layered.paint)(hdc, client);
    unsafe {
        let _ = SetViewportOrgEx(hdc, 0, 0, None);
    }
    // GDI 只写 RGB、把碰到的像素 alpha 留成 0（分层窗口里会全透明），画完把内容区补回 255。
    restore_content_alpha(canvas.pixels(), w, h, &round);
    if let Some(overlay) = overlay {
        blend_overlay(canvas.pixels(), w, h, &overlay);
    }
    #[cfg(test)]
    preview_export::export_once(canvas.pixels(), w, h)?;
    update(hwnd, canvas.dc(), layered.win_pos, (w, h))
}

/// 把渲染器出的预乘 RGBA 位图（已含阴影边）贴到分层窗口上，`win_pos` 是位图左上角的屏幕坐标。
pub(super) fn present(hwnd: HWND, pixmap: &Pixmap, win_pos: (i32, i32)) -> Result<()> {
    present_with_overlay(hwnd, pixmap, win_pos, None)
}

/// 把渲染器位图与可选角标一起贴到分层窗口。
pub(super) fn present_with_overlay(
    hwnd: HWND,
    pixmap: &Pixmap,
    win_pos: (i32, i32),
    overlay: Option<PixmapOverlay<'_>>,
) -> Result<()> {
    let (w, h) = (pixmap.width() as i32, pixmap.height() as i32);
    if w <= 0 || h <= 0 {
        return Err(Error::from(E_INVALIDARG));
    }
    let started = std::time::Instant::now();
    let mut canvas = Canvas::new(w, h)?;
    // tiny-skia 是 RGBA，DIB 是 BGRA；都是预乘，只换通道顺序。
    for (dst, src) in canvas
        .pixels()
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(pixmap.pixels())
    {
        dst[0] = src.blue();
        dst[1] = src.green();
        dst[2] = src.red();
        dst[3] = src.alpha();
    }
    if let Some(overlay) = overlay {
        blend_overlay(canvas.pixels(), w, h, &overlay);
    }
    let elapsed = started.elapsed();
    if elapsed >= std::time::Duration::from_millis(5) {
        tracing::debug!(elapsed = ?elapsed, width = w, height = h, "候选窗位图转换慢帧");
    }
    #[cfg(test)]
    preview_export::export_once(canvas.pixels(), w, h)?;
    update(hwnd, canvas.dc(), win_pos, (w, h))
}

fn blend_overlay(pixels: &mut [u8], width: i32, height: i32, overlay: &PixmapOverlay<'_>) {
    for (index, source) in overlay.pixmap.pixels().iter().enumerate() {
        if source.alpha() == 0 {
            continue;
        }
        let x = overlay.x + (index as u32 % overlay.pixmap.width()) as i32;
        let y = overlay.y + (index as u32 / overlay.pixmap.width()) as i32;
        if x < 0 || y < 0 || x >= width || y >= height {
            continue;
        }
        let offset = ((y * width + x) * 4) as usize;
        let destination = &mut pixels[offset..offset + 4];
        let inverse = 255 - source.alpha() as u16;
        for (channel, value) in [source.blue(), source.green(), source.red()]
            .into_iter()
            .enumerate()
        {
            destination[channel] =
                value.saturating_add(((destination[channel] as u16 * inverse + 127) / 255) as u8);
        }
        destination[3] =
            (source.alpha() as u16 + (destination[3] as u16 * inverse + 127) / 255).min(255) as u8;
    }
}

/// `UpdateLayeredWindow`：整张位图按预乘 alpha 贴上并挪到 `win_pos`。
fn update(hwnd: HWND, hdc: HDC, win_pos: (i32, i32), win_size: (i32, i32)) -> Result<()> {
    let started = std::time::Instant::now();
    let (w, h) = win_size;
    let dst = POINT {
        x: win_pos.0,
        y: win_pos.1,
    };
    let size = SIZE { cx: w, cy: h };
    let src = POINT { x: 0, y: 0 };
    let blend = BLENDFUNCTION {
        BlendOp: AC_SRC_OVER as u8,
        BlendFlags: 0,
        SourceConstantAlpha: 255,
        AlphaFormat: AC_SRC_ALPHA as u8,
    };
    let result = unsafe {
        UpdateLayeredWindow(
            hwnd,
            None,
            Some(&dst),
            Some(&size),
            Some(hdc),
            Some(&src),
            COLORREF(0),
            Some(&blend),
            ULW_ALPHA,
        )
    };
    let elapsed = started.elapsed();
    if elapsed >= std::time::Duration::from_millis(5) {
        tracing::debug!(elapsed = ?elapsed, width = w, height = h, "分层窗口提交慢帧");
    }
    result
}

/// 圆角矩形内填渐变背景（alpha 255），外面画黑色阴影。阴影纯黑、背景不透明，所以不用真做预乘。
/// 阴影从内容边缘就开始淡出，不偏移、不填实心色带（否则底边多一道生硬暗带）。
fn fill_shadow_and_background(
    pixels: &mut [u8],
    w: i32,
    h: i32,
    round: &RoundRect,
    background: (COLORREF, COLORREF),
    margin: i32,
) {
    // COLORREF 低位到高位是 R、G、B；DIB 每像素是 B、G、R、A。
    let bg_start = color_channels(background.0);
    let bg_end = color_channels(background.1);
    let content_width = (w - margin * 2).max(1) as f64;
    let content_height = (h - margin * 2).max(1) as f64;
    let gradient_length = content_width.powi(2) + content_height.powi(2);
    let margin = (margin as f64).max(1.0);
    for y in 0..h {
        for x in 0..w {
            let idx = ((y * w + x) * 4) as usize;
            let d = round.distance(x, y);
            if d <= 0.0 {
                // 投影到左上到右下的对角线，和 tiny-skia 画笔使用同一条渐变方向。
                let t = (((x as f64 + 0.5 - margin) * content_width
                    + (y as f64 + 0.5 - margin) * content_height)
                    / gradient_length)
                    .clamp(0.0, 1.0);
                for channel in 0..3 {
                    pixels[idx + channel] = (bg_start[channel]
                        + (bg_end[channel] - bg_start[channel]) * t)
                        .round() as u8;
                }
                pixels[idx + 3] = 255;
            } else {
                let fall = (1.0 - d / margin).max(0.0);
                let fall = fall * fall;
                let ambient = AMBIENT_MAX * fall;
                let key = KEY_MAX * round.down_weight(x, y) * fall;
                pixels[idx] = 0;
                pixels[idx + 1] = 0;
                pixels[idx + 2] = 0;
                pixels[idx + 3] = (ambient + key).min(255.0) as u8;
            }
        }
    }
}

/// COLORREF 转为 DIB 使用的 B、G、R 顺序。
fn color_channels(color: COLORREF) -> [f64; 3] {
    [
        ((color.0 >> 16) & 0xff) as f64,
        ((color.0 >> 8) & 0xff) as f64,
        (color.0 & 0xff) as f64,
    ]
}

/// 把内容区所有像素 alpha 补成 255。
fn restore_content_alpha(pixels: &mut [u8], w: i32, h: i32, round: &RoundRect) {
    for y in 0..h {
        for x in 0..w {
            if round.distance(x, y) <= 0.0 {
                let idx = ((y * w + x) * 4) as usize;
                pixels[idx + 3] = 255;
            }
        }
    }
}
