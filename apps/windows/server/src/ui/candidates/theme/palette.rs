//! Windows 品牌紫色配色；与 Miracle 渲染器的品牌主题保持一致。

use windows::Win32::Foundation::COLORREF;

use super::rgb;

/// GDI 绘制用的不透明配色。
pub(super) struct Palette {
    pub(super) text_color: COLORREF,
    pub(super) gloss_color: COLORREF,
    pub(super) pos_color: COLORREF,
    pub(super) fresh_color: COLORREF,
    pub(super) index_color: COLORREF,
    pub(super) cloud_color: COLORREF,
    pub(super) background: COLORREF,
    pub(super) background_end: COLORREF,
    pub(super) highlight: COLORREF,
}

impl Palette {
    pub(super) fn purple() -> Self {
        Self {
            text_color: rgb(0xff, 0xff, 0xff),
            gloss_color: rgb(0xe5, 0xd8, 0xf8),
            pos_color: rgb(0xdc, 0xcb, 0xf3),
            fresh_color: rgb(0xff, 0xe4, 0xb2),
            index_color: rgb(0xdc, 0xcb, 0xf3),
            cloud_color: rgb(0xf7, 0xee, 0xff),
            background: rgb(0x6d, 0x35, 0xb5),
            background_end: rgb(0x35, 0x20, 0x63),
            // GDI 不支持颜色 alpha，选中项用略亮的紫色。
            highlight: rgb(0x80, 0x57, 0xb5),
        }
    }
}
