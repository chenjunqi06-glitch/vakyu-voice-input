//! 线程内最近一项模糊遮罩与可复用工作区；尺寸变化后仍保留缓冲容量。

use tiny_skia::{IntSize, Pixmap, Rect};

use super::Shadow;
use super::blur::{horizontal, vertical};
use super::key::ShadowKey;
use crate::canvas::Canvas;
use crate::color::Color;

#[derive(Default)]
pub(super) struct ShadowScratch {
    key: Option<ShadowKey>,

    rgba: Vec<u8>,

    mask: Vec<u8>,

    row: Vec<u8>,

    /// 垂直模糊按行写到另一张遮罩，完成后与 mask 交换。
    output: Vec<u8>,

    sums: Vec<u32>,
}

impl ShadowScratch {
    pub(super) fn mask(&self) -> &[u8] {
        &self.mask
    }

    /// 返回是否重建；尺寸无法表示为 tiny-skia 位图时返回 `None`。
    pub(super) fn prepare_mask(
        &mut self,
        size: (u32, u32),
        content: Rect,
        radius: f32,
        scale: f32,
        shadow: &Shadow,
    ) -> Option<bool> {
        let key = ShadowKey::new(size, content, radius, scale, shadow);
        if self.key == Some(key) {
            return Some(false);
        }

        let pixmap_size = IntSize::from_wh(size.0, size.1)?;
        let pixel_count = (size.0 as usize).checked_mul(size.1 as usize)?;
        self.rgba.resize(pixel_count.checked_mul(4)?, 0);
        self.rgba.fill(0);
        let pixmap = Pixmap::from_vec(std::mem::take(&mut self.rgba), pixmap_size)?;
        let mut mask_canvas = Canvas::from_pixmap(pixmap);
        mask_canvas.fill_round_rect(
            content.x(),
            content.y() + shadow.offset_y * scale,
            content.width(),
            content.height(),
            radius,
            Color::rgb(0, 0, 0),
        );
        self.rgba = mask_canvas.into_pixmap().take();

        self.mask.resize(pixel_count, 0);
        for (coverage, pixel) in self.mask.iter_mut().zip(self.rgba.as_chunks::<4>().0) {
            *coverage = pixel[3];
        }
        self.row.resize(size.0 as usize, 0);
        self.output.resize(pixel_count, 0);
        self.sums.resize(size.0 as usize, 0);
        let radius_px = (shadow.blur * scale / 2.0).round().max(1.0) as usize;
        for _ in 0..3 {
            horizontal(
                &mut self.mask,
                &mut self.row,
                size.0 as usize,
                size.1 as usize,
                radius_px,
            );
            vertical(
                &self.mask,
                &mut self.output,
                &mut self.sums,
                size.0 as usize,
                size.1 as usize,
                radius_px,
            );
            std::mem::swap(&mut self.mask, &mut self.output);
        }
        self.key = Some(key);
        Some(true)
    }
}
