//! 阴影外观参数与绘制入口；macOS 可交给系统，Windows 分层窗口使用自绘阴影。

use std::cell::RefCell;

use tiny_skia::Rect;

use super::scratch::ShadowScratch;
use crate::canvas::Canvas;
use crate::color::Color;

thread_local! {
    // 同一 UI 线程共享最近一项遮罩；几何未变时不重新栅格化或模糊。
    static SCRATCH: RefCell<ShadowScratch> = RefCell::new(ShadowScratch::default());
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shadow {
    /// 模糊半径（点）。
    pub blur: f32,

    /// 向下偏移（点）。
    pub offset_y: f32,

    /// 颜色（含不透明度）。
    pub color: Color,
}

impl Shadow {
    /// 接近 macOS 弹出面板的系统阴影。
    pub const fn mac_panel() -> Self {
        Self {
            blur: 12.0,
            offset_y: 6.0,
            color: Color::gray(0, 90),
        }
    }

    /// 阴影在内容四周要留多宽的边（点）。
    pub(crate) fn margin(&self) -> f32 {
        self.blur * 2.0 + self.offset_y
    }

    /// 在 `canvas` 上给 `content` 这块圆角内容画阴影；坐标都是像素。
    pub(crate) fn paint(&self, canvas: &mut Canvas, content: Rect, radius: f32, scale: f32) {
        let (width, height) = (canvas.width(), canvas.height());
        SCRATCH.with(|scratch| {
            let mut scratch = scratch.borrow_mut();
            if scratch
                .prepare_mask((width, height), content, radius, scale, self)
                .is_none()
            {
                return;
            }
            canvas.blend_mask(0, 0, width, height, scratch.mask(), self.color);
        });
    }
}
