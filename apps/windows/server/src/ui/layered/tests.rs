//! 分层位图的渐变与透明区域验证。

use windows::Win32::Foundation::COLORREF;

use super::round_rect::RoundRect;
use super::{PixmapOverlay, blend_overlay, fill_shadow_and_background};
use qingjian_render::Pixmap;
use tiny_skia::PremultipliedColorU8;

#[test]
fn background_gradient_preserves_opaque_content_and_translucent_shadow() {
    let (w, h, margin) = (120, 80, 10);
    let mut pixels = vec![0; w as usize * h as usize * 4];
    let round = RoundRect::content((100, 60), margin, 12);
    fill_shadow_and_background(
        &mut pixels,
        w,
        h,
        &round,
        (COLORREF(0xb5356d), COLORREF(0x632035)),
        margin,
    );
    let pixel = |x: usize, y: usize| &pixels[(y * w as usize + x) * 4..][..4];
    assert_eq!(pixel(0, 0)[3], 0);
    assert!(pixel(60, 74)[3] > 0 && pixel(60, 74)[3] < 255);
    let start = pixel(24, 24);
    let end = pixel(94, 54);
    assert_eq!((start[3], end[3]), (255, 255));
    assert!(start[0] > end[0]);
    assert!(start[2] > end[2]);
    assert!(start[0] > start[2]);
    assert!(end[0] > end[2]);
}

#[test]
fn overlay_uses_alpha_and_clips_outside_the_layered_bitmap() {
    let mut target = vec![0; 2 * 2 * 4];
    let mut mark = Pixmap::new(2, 1).unwrap();
    mark.pixels_mut()[0] = PremultipliedColorU8::from_rgba(100, 20, 0, 128).unwrap();
    mark.pixels_mut()[1] = PremultipliedColorU8::from_rgba(0, 0, 255, 255).unwrap();

    blend_overlay(
        &mut target,
        2,
        2,
        &PixmapOverlay {
            pixmap: &mark,
            x: 1,
            y: 0,
        },
    );

    assert_eq!(&target[0..4], &[0, 0, 0, 0]);
    // 输入已预乘 alpha，透明目标上只换 RGBA→BGRA，不能重复乘透明度。
    assert_eq!(&target[4..8], &[0, 20, 100, 128]);
}
