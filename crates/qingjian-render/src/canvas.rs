//! 位图画布：tiny-skia `Pixmap` 之上的几个填充原语，加字形位图的逐像素 source-over 混合。坐标一律是像素、左上角原点。

use tiny_skia::{
    BlendMode, FillRule, GradientStop, LinearGradient, Paint, Path, PathBuilder, Pixmap, Point,
    PremultipliedColorU8, Rect, SpreadMode, Transform,
};

use crate::color::{Color, mul_u8, premultiply};
use crate::error::RenderError;

pub(crate) struct Canvas {
    /// 预乘 RGBA。
    pixmap: Pixmap,
}

impl Canvas {
    pub(crate) fn new(width: u32, height: u32) -> Result<Self, RenderError> {
        Pixmap::new(width, height)
            .map(|pixmap| Self { pixmap })
            .ok_or(RenderError::InvalidSize { width, height })
    }

    pub(crate) fn from_pixmap(pixmap: Pixmap) -> Self {
        Self { pixmap }
    }

    pub(crate) fn width(&self) -> u32 {
        self.pixmap.width()
    }

    pub(crate) fn height(&self) -> u32 {
        self.pixmap.height()
    }

    pub(crate) fn into_pixmap(self) -> Pixmap {
        self.pixmap
    }

    pub(crate) fn fill_rect(&mut self, x: f32, y: f32, width: f32, height: f32, color: Color) {
        let Some(rect) = Rect::from_xywh(x, y, width, height) else {
            return;
        };
        self.pixmap.fill_rect(
            rect,
            &paint(color, BlendMode::SourceOver),
            Transform::identity(),
            None,
        );
    }

    pub(crate) fn fill_round_rect(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        radius: f32,
        color: Color,
    ) {
        if let Some(path) = round_rect(x, y, width, height, radius) {
            self.fill_path(&path, color, BlendMode::SourceOver);
        }
    }

    /// 圆角背景；可选左上到右下渐变，阴影和圆角外透明像素不受影响。
    pub(crate) fn fill_background(
        &mut self,
        rect: Rect,
        radius: f32,
        start: Color,
        end: Option<Color>,
    ) {
        let Some(end) = end else {
            self.fill_round_rect(
                rect.x(),
                rect.y(),
                rect.width(),
                rect.height(),
                radius,
                start,
            );
            return;
        };
        let Some(path) = round_rect(rect.x(), rect.y(), rect.width(), rect.height(), radius) else {
            return;
        };
        let Some(shader) = LinearGradient::new(
            Point::from_xy(rect.left(), rect.top()),
            Point::from_xy(rect.right(), rect.bottom()),
            vec![
                GradientStop::new(0.0, start.to_skia()),
                GradientStop::new(1.0, end.to_skia()),
            ],
            SpreadMode::Pad,
            Transform::identity(),
        ) else {
            return;
        };
        let paint = Paint {
            shader,
            anti_alias: true,
            ..Paint::default()
        };
        self.pixmap.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    pub(crate) fn fill_path(&mut self, path: &Path, color: Color, blend: BlendMode) {
        self.pixmap.fill_path(
            path,
            &paint(color, blend),
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    /// 把另一张位图整张叠上来（左上角对齐到 `(x, y)`）。
    pub(crate) fn blend_pixmap(&mut self, x: i32, y: i32, other: &Pixmap) {
        let width = other.width();
        for (i, pixel) in other.pixels().iter().enumerate() {
            if pixel.alpha() == 0 {
                continue;
            }
            let px = x + (i as u32 % width) as i32;
            let py = y + (i as u32 / width) as i32;
            self.blend_pixel(px, py, *pixel);
        }
    }

    /// 8 位覆盖率遮罩（普通字形）按颜色叠上来。
    pub(crate) fn blend_mask(
        &mut self,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        data: &[u8],
        color: Color,
    ) {
        for row in 0..height {
            for col in 0..width {
                let Some(&coverage) = data.get((row * width + col) as usize) else {
                    return;
                };
                if coverage == 0 {
                    continue;
                }
                self.blend_pixel(
                    x + col as i32,
                    y + row as i32,
                    color.premultiplied(coverage),
                );
            }
        }
    }

    /// 用覆盖率查找表绘制字形遮罩，避免为了 gamma 校正复制并分配整张字形位图。
    pub(crate) fn blend_mask_lut(
        &mut self,
        x: i32,
        y: i32,
        size: (u32, u32),
        data: &[u8],
        color: Color,
        lut: &[u8; 256],
    ) {
        let (width, height) = size;
        for row in 0..height {
            for col in 0..width {
                let Some(&coverage) = data.get((row * width + col) as usize) else {
                    return;
                };
                let coverage = lut[coverage as usize];
                if coverage == 0 {
                    continue;
                }
                self.blend_pixel(
                    x + col as i32,
                    y + row as i32,
                    color.premultiplied(coverage),
                );
            }
        }
    }

    /// 直通 RGBA 位图（彩色 emoji）叠上来。
    pub(crate) fn blend_rgba(&mut self, x: i32, y: i32, width: u32, height: u32, data: &[u8]) {
        for (i, px) in data.as_chunks::<4>().0.iter().enumerate() {
            if px[3] == 0 {
                continue;
            }
            let col = (i as u32 % width) as i32;
            let row = (i as u32 / width) as i32;
            if row >= height as i32 {
                return;
            }
            self.blend_pixel(x + col, y + row, premultiply(px[0], px[1], px[2], px[3]));
        }
    }

    /// source-over：`dst = src + dst × (1 − src.a)`。
    fn blend_pixel(&mut self, x: i32, y: i32, src: PremultipliedColorU8) {
        if x < 0 || y < 0 || x >= self.pixmap.width() as i32 || y >= self.pixmap.height() as i32 {
            return;
        }
        let index = y as usize * self.pixmap.width() as usize + x as usize;
        let dst = &mut self.pixmap.pixels_mut()[index];
        let inverse = 255 - src.alpha();
        let a = src.alpha().saturating_add(mul_u8(dst.alpha(), inverse));
        let channel = |s: u8, d: u8| s.saturating_add(mul_u8(d, inverse)).min(a);
        let r = channel(src.red(), dst.red());
        let g = channel(src.green(), dst.green());
        let b = channel(src.blue(), dst.blue());
        if let Some(out) = PremultipliedColorU8::from_rgba(r, g, b, a) {
            *dst = out;
        }
    }
}

fn paint(color: Color, blend: BlendMode) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color(color.to_skia());
    paint.anti_alias = true;
    paint.blend_mode = blend;
    paint
}

/// 圆角矩形路径；圆角用三次贝塞尔近似四分之一圆。
pub(crate) fn round_rect(x: f32, y: f32, width: f32, height: f32, radius: f32) -> Option<Path> {
    let r = radius.min(width / 2.0).min(height / 2.0).max(0.0);
    if r <= 0.0 {
        return Rect::from_xywh(x, y, width, height).map(PathBuilder::from_rect);
    }
    // 四分之一圆的贝塞尔控制点系数
    const KAPPA: f32 = 0.552_284_8;
    let k = r * KAPPA;
    let (right, bottom) = (x + width, y + height);
    let mut path = PathBuilder::new();
    path.move_to(x + r, y);
    path.line_to(right - r, y);
    path.cubic_to(right - r + k, y, right, y + r - k, right, y + r);
    path.line_to(right, bottom - r);
    path.cubic_to(
        right,
        bottom - r + k,
        right - r + k,
        bottom,
        right - r,
        bottom,
    );
    path.line_to(x + r, bottom);
    path.cubic_to(x + r - k, bottom, x, bottom - r + k, x, bottom - r);
    path.line_to(x, y + r);
    path.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    path.close();
    path.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gradient_background_retains_transparent_corners_and_changes_diagonally() {
        let mut canvas = Canvas::new(120, 80).unwrap();
        canvas.fill_background(
            Rect::from_xywh(10.0, 10.0, 100.0, 60.0).unwrap(),
            12.0,
            Color::rgb(109, 53, 181),
            Some(Color::rgb(53, 32, 99)),
        );
        let pixmap = canvas.into_pixmap();
        assert_eq!(pixmap.pixel(0, 0).unwrap().alpha(), 0);
        let start = pixmap.pixel(24, 24).unwrap();
        let end = pixmap.pixel(94, 54).unwrap();
        assert_eq!((start.alpha(), end.alpha()), (255, 255));
        assert!(start.red() > end.red());
        assert!(start.blue() > end.blue());
        assert!(start.blue() > start.red());
        assert!(end.blue() > end.red());
    }

    #[test]
    fn blends_source_over_and_clips_to_bounds() {
        let mut canvas = Canvas::new(2, 1).unwrap();
        canvas.fill_rect(0.0, 0.0, 2.0, 1.0, Color::rgb(0, 0, 255));
        canvas.blend_mask(0, 0, 1, 1, &[255], Color::rgb(255, 0, 0));
        canvas.blend_mask(5, 5, 1, 1, &[255], Color::rgb(255, 0, 0));
        let pixmap = canvas.into_pixmap();
        let red = pixmap.pixel(0, 0).unwrap();
        assert_eq!((red.red(), red.blue(), red.alpha()), (255, 0, 255));
        let blue = pixmap.pixel(1, 0).unwrap();
        assert_eq!((blue.red(), blue.blue()), (0, 255));
    }

    #[test]
    fn half_alpha_over_white_lightens() {
        let mut canvas = Canvas::new(1, 1).unwrap();
        canvas.fill_rect(0.0, 0.0, 1.0, 1.0, Color::rgb(255, 255, 255));
        canvas.blend_mask(0, 0, 1, 1, &[255], Color::gray(0, 128));
        let px = canvas.into_pixmap().pixel(0, 0).unwrap();
        assert_eq!(px.alpha(), 255);
        assert!((px.red() as i32 - 127).abs() <= 1, "got {}", px.red());
    }

    #[test]
    fn mask_lut_matches_pretransformed_coverage() {
        let mut lut = [0u8; 256];
        for (index, value) in lut.iter_mut().enumerate() {
            *value = (index / 2) as u8;
        }
        let source = [0, 64, 128, 255];
        let transformed = source.map(|coverage| lut[coverage as usize]);
        let mut direct = Canvas::new(4, 1).unwrap();
        let mut pretransformed = Canvas::new(4, 1).unwrap();
        direct.blend_mask_lut(0, 0, (4, 1), &source, Color::rgb(120, 40, 200), &lut);
        pretransformed.blend_mask(0, 0, 4, 1, &transformed, Color::rgb(120, 40, 200));
        assert_eq!(
            direct.into_pixmap().pixels(),
            pretransformed.into_pixmap().pixels()
        );
    }
}
