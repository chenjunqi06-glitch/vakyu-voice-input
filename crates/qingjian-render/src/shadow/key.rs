//! 阴影遮罩的几何键；颜色在混合时应用，改颜色无需重建遮罩。

use tiny_skia::Rect;

use super::Shadow;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ShadowKey {
    width: u32,

    height: u32,

    /// 所有浮点参数保留原始位模式，避免近似匹配用到旧几何。
    geometry: [u32; 8],
}

impl ShadowKey {
    pub(super) fn new(
        size: (u32, u32),
        content: Rect,
        radius: f32,
        scale: f32,
        shadow: &Shadow,
    ) -> Self {
        Self {
            width: size.0,
            height: size.1,
            geometry: [
                content.x().to_bits(),
                content.y().to_bits(),
                content.width().to_bits(),
                content.height().to_bits(),
                radius.to_bits(),
                scale.to_bits(),
                shadow.blur.to_bits(),
                shadow.offset_y.to_bits(),
            ],
        }
    }
}
