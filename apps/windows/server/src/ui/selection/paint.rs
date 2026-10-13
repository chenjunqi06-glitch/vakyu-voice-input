//! 「译」小图标与译文小窗的 GDI 绘制与排版；坐标都是物理像素，按 DPI 缩放。

use windows::Win32::Foundation::{COLORREF, HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateFontW, CreateRoundRectRgn,
    CreateSolidBrush, DEFAULT_CHARSET, DRAW_TEXT_FORMAT, DT_CALCRECT, DT_CENTER, DT_EDITCONTROL,
    DT_LEFT, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, DT_WORDBREAK, DeleteObject, DrawTextW,
    EndPaint, FF_DONTCARE, FillRect, FillRgn, GetDC, HDC, HFONT, OUT_TT_PRECIS, PAINTSTRUCT,
    ReleaseDC, SelectObject, SetBkMode, SetTextColor, TRANSPARENT, VARIABLE_PITCH,
};
use windows::core::w;

/// 小窗内容。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct PopupContent {
    pub title: String,

    /// 选中的原文（截短后显示两行以内）。
    pub source: String,

    pub text: String,

    /// 译文已到，可以复制。
    pub ready: bool,

    pub failed: bool,

    pub copied: bool,
}

/// 小窗排版结果：窗口尺寸与两个按钮的位置（窗口客户区坐标）。
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct PopupLayout {
    pub width: i32,

    pub height: i32,

    pub close: RECT,

    pub copy: RECT,

    source: RECT,

    text: RECT,

    /// 译文太长、被截掉了一部分。
    clipped: bool,
}

const BRAND: COLORREF = rgb(104, 56, 194);
const BACKGROUND: COLORREF = rgb(250, 249, 255);
const TEXT: COLORREF = rgb(32, 30, 44);
const MUTED: COLORREF = rgb(112, 108, 128);
const ERROR: COLORREF = rgb(186, 64, 40);
const WHITE: COLORREF = rgb(255, 255, 255);

const fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    COLORREF((r as u32) | ((g as u32) << 8) | ((b as u32) << 16))
}

pub(super) fn scale(value: i32, dpi: u32) -> i32 {
    (value * dpi as i32 + 48) / 96
}

/// 按 DPI 造的一套字体；GDI 资源，`Drop` 里删。
pub(super) struct Fonts {
    pub dpi: u32,

    title: HFONT,

    body: HFONT,

    small: HFONT,

    mark: HFONT,
}

impl Fonts {
    pub(super) fn new(dpi: u32) -> Self {
        Self {
            dpi,
            title: font(-scale(14, dpi), 700),
            body: font(-scale(15, dpi), 400),
            small: font(-scale(12, dpi), 400),
            mark: font(-scale(15, dpi), 700),
        }
    }
}

impl Drop for Fonts {
    fn drop(&mut self) {
        for font in [self.title, self.body, self.small, self.mark] {
            if !font.is_invalid() {
                let _ = unsafe { DeleteObject(font.into()) };
            }
        }
    }
}

fn font(height: i32, weight: i32) -> HFONT {
    unsafe {
        CreateFontW(
            height,
            0,
            0,
            0,
            weight,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_TT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            (VARIABLE_PITCH.0 | FF_DONTCARE.0) as u32,
            w!("Microsoft YaHei UI"),
        )
    }
}

/// 图标边长（物理像素）。
pub(super) fn icon_size(dpi: u32) -> i32 {
    scale(28, dpi)
}

pub(super) fn paint_icon(hwnd: HWND, fonts: &Fonts) {
    let mut paint = PAINTSTRUCT::default();
    let hdc = unsafe { BeginPaint(hwnd, &mut paint) };
    let size = icon_size(fonts.dpi);
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: size,
        bottom: size,
    };
    fill(hdc, rect, BRAND);
    text(
        hdc,
        fonts.mark,
        WHITE,
        "译",
        &mut rect,
        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
    );
    let _ = unsafe { EndPaint(hwnd, &paint) };
}

/// 给窗口套上圆角；系统接管区域句柄，失败时自己删。
pub(super) fn round_corners(hwnd: HWND, width: i32, height: i32, radius: i32) {
    use windows::Win32::Graphics::Gdi::SetWindowRgn;
    let region = unsafe { CreateRoundRectRgn(0, 0, width + 1, height + 1, radius * 2, radius * 2) };
    if !region.is_invalid() && unsafe { SetWindowRgn(hwnd, Some(region), true) } == 0 {
        let _ = unsafe { DeleteObject(region.into()) };
    }
}

/// 按内容排版：宽度固定，高度随译文增长，超过上限截断并在页脚提示。
pub(super) fn layout_popup(hwnd: HWND, fonts: &Fonts, content: &PopupContent) -> PopupLayout {
    let dpi = fonts.dpi;
    let width = scale(380, dpi);
    let pad = scale(14, dpi);
    let inner = width - pad * 2;
    let title_h = scale(22, dpi);
    let button_h = scale(28, dpi);
    let max_text = scale(300, dpi);
    let hdc = unsafe { GetDC(Some(hwnd)) };
    let source_h =
        measure(hdc, fonts.small, &short_source(&content.source), inner).min(scale(36, dpi));
    let full_text = measure(hdc, fonts.body, &content.text, inner).max(scale(20, dpi));
    unsafe { ReleaseDC(Some(hwnd), hdc) };
    let text_h = full_text.min(max_text);
    let mut y = pad;
    let close = RECT {
        left: width - pad - title_h,
        top: y,
        right: width - pad,
        bottom: y + title_h,
    };
    y += title_h + scale(6, dpi);
    let source = RECT {
        left: pad,
        top: y,
        right: width - pad,
        bottom: y + source_h,
    };
    y += source_h + scale(8, dpi);
    let text = RECT {
        left: pad,
        top: y,
        right: width - pad,
        bottom: y + text_h,
    };
    y += text_h + scale(12, dpi);
    let copy = RECT {
        left: width - pad - scale(96, dpi),
        top: y,
        right: width - pad,
        bottom: y + button_h,
    };
    y += button_h + pad;
    PopupLayout {
        width,
        height: y,
        close,
        copy,
        source,
        text,
        clipped: full_text > max_text,
    }
}

pub(super) fn paint_popup(hwnd: HWND, fonts: &Fonts, content: &PopupContent, layout: &PopupLayout) {
    let mut paint = PAINTSTRUCT::default();
    let hdc = unsafe { BeginPaint(hwnd, &mut paint) };
    let dpi = fonts.dpi;
    let border = scale(1, dpi).max(1);
    let radius = scale(12, dpi);
    fill(
        hdc,
        RECT {
            left: 0,
            top: 0,
            right: layout.width,
            bottom: layout.height,
        },
        BRAND,
    );
    fill_round(
        hdc,
        RECT {
            left: border,
            top: border,
            right: layout.width - border,
            bottom: layout.height - border,
        },
        BACKGROUND,
        radius,
    );
    let pad = scale(14, dpi);
    let mut title = RECT {
        left: pad,
        top: layout.close.top,
        right: layout.close.left - scale(6, dpi),
        bottom: layout.close.bottom,
    };
    text(
        hdc,
        fonts.title,
        BRAND,
        &content.title,
        &mut title,
        DT_LEFT | DT_VCENTER | DT_SINGLELINE,
    );
    let mut close = layout.close;
    text(
        hdc,
        fonts.title,
        MUTED,
        "×",
        &mut close,
        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
    );
    let mut source = layout.source;
    text(
        hdc,
        fonts.small,
        MUTED,
        &short_source(&content.source),
        &mut source,
        DT_WORDBREAK | DT_EDITCONTROL,
    );
    let mut body = layout.text;
    let color = if content.failed { ERROR } else { TEXT };
    text(
        hdc,
        fonts.body,
        color,
        &content.text,
        &mut body,
        DT_WORDBREAK | DT_EDITCONTROL,
    );
    let mut hint = RECT {
        left: pad,
        top: layout.copy.top,
        right: layout.copy.left - scale(8, dpi),
        bottom: layout.copy.bottom,
    };
    let note = if layout.clipped {
        "译文较长，复制可得全文"
    } else {
        "只读取选中的文字，不改动原文"
    };
    text(
        hdc,
        fonts.small,
        MUTED,
        note,
        &mut hint,
        DT_LEFT | DT_VCENTER | DT_SINGLELINE,
    );
    let enabled = content.ready && !content.failed;
    fill_round(
        hdc,
        layout.copy,
        if enabled { BRAND } else { rgb(200, 196, 214) },
        scale(6, dpi),
    );
    let mut button = layout.copy;
    let label = if content.copied {
        "已复制"
    } else {
        "复制译文"
    };
    text(
        hdc,
        fonts.small,
        WHITE,
        label,
        &mut button,
        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
    );
    let _ = unsafe { EndPaint(hwnd, &paint) };
}

/// 原文只显示开头一段，避免长选区把小窗撑满。
fn short_source(source: &str) -> String {
    let flat: String = source.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = flat.chars();
    let short: String = chars.by_ref().take(80).collect();
    if chars.next().is_some() {
        format!("{short}…")
    } else {
        short
    }
}

fn measure(hdc: HDC, font: HFONT, value: &str, width: i32) -> i32 {
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: width,
        bottom: 0,
    };
    text(
        hdc,
        font,
        TEXT,
        value,
        &mut rect,
        DT_WORDBREAK | DT_EDITCONTROL | DT_CALCRECT,
    );
    rect.bottom - rect.top
}

fn text(
    hdc: HDC,
    font: HFONT,
    color: COLORREF,
    value: &str,
    rect: &mut RECT,
    format: DRAW_TEXT_FORMAT,
) {
    let mut utf16: Vec<u16> = value.encode_utf16().collect();
    if utf16.is_empty() {
        return;
    }
    unsafe {
        let previous = SelectObject(hdc, font.into());
        SetBkMode(hdc, TRANSPARENT);
        SetTextColor(hdc, color);
        DrawTextW(hdc, &mut utf16, rect, format | DT_NOPREFIX);
        SelectObject(hdc, previous);
    }
}

fn fill(hdc: HDC, rect: RECT, color: COLORREF) {
    unsafe {
        let brush = CreateSolidBrush(color);
        FillRect(hdc, &rect, brush);
        let _ = DeleteObject(brush.into());
    }
}

fn fill_round(hdc: HDC, rect: RECT, color: COLORREF, radius: i32) {
    unsafe {
        let region = CreateRoundRectRgn(
            rect.left,
            rect.top,
            rect.right,
            rect.bottom,
            radius * 2,
            radius * 2,
        );
        let brush = CreateSolidBrush(color);
        let _ = FillRgn(hdc, region, brush);
        let _ = DeleteObject(brush.into());
        let _ = DeleteObject(region.into());
    }
}

#[cfg(test)]
mod tests {
    use super::{scale, short_source};

    #[test]
    fn long_sources_are_shortened_and_flattened() {
        assert_eq!(short_source("a\n b"), "a b");
        let long = "字".repeat(100);
        let short = short_source(&long);
        assert_eq!(short.chars().count(), 81);
        assert!(short.ends_with('…'));
        assert_eq!(scale(28, 96), 28);
        assert_eq!(scale(28, 144), 42);
    }
}
