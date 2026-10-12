//! 阴影缓存与无缓存绘制结果的一致性，以及几何和颜色变化的失效边界。

use tiny_skia::Rect;

use super::Shadow;
use super::blur::line;
use super::scratch::ShadowScratch;
use crate::canvas::Canvas;
use crate::color::Color;

fn shadow() -> Shadow {
    Shadow {
        blur: 3.0,
        offset_y: 2.0,
        color: Color::gray(0, 90),
    }
}

fn content() -> Rect {
    Rect::from_xywh(8.0, 7.0, 22.0, 14.0).unwrap()
}

#[test]
fn blur_spreads_and_preserves_mass() {
    let mut dst = [0u8; 5];
    line(&[0, 0, 255, 0, 0], &mut dst, 1);
    assert_eq!(dst, [0, 85, 85, 85, 0]);
}

#[test]
fn cached_mask_hit_reuses_same_coverage_and_keeps_output() {
    let mut scratch = ShadowScratch::default();
    let shadow = shadow();
    assert_eq!(
        scratch.prepare_mask((48, 40), content(), 4.0, 1.0, &shadow),
        Some(true)
    );
    let first_mask = scratch.mask().to_vec();
    assert_eq!(
        scratch.prepare_mask((48, 40), content(), 4.0, 1.0, &shadow),
        Some(false)
    );
    assert_eq!(scratch.mask(), first_mask);

    let mut first = Canvas::new(48, 40).unwrap();
    let mut second = Canvas::new(48, 40).unwrap();
    first.blend_mask(0, 0, 48, 40, &first_mask, shadow.color);
    second.blend_mask(0, 0, 48, 40, scratch.mask(), shadow.color);
    assert_eq!(first.into_pixmap().pixels(), second.into_pixmap().pixels());
}

#[test]
fn geometry_changes_rebuild_mask_and_match_fresh_rendering() {
    let original = shadow();
    let cases = [
        ((49, 40), content(), 4.0, 1.0, original),
        ((48, 41), content(), 4.0, 1.0, original),
        (
            (48, 40),
            Rect::from_xywh(9.0, 7.0, 22.0, 14.0).unwrap(),
            4.0,
            1.0,
            original,
        ),
        (
            (48, 40),
            Rect::from_xywh(8.0, 8.0, 22.0, 14.0).unwrap(),
            4.0,
            1.0,
            original,
        ),
        (
            (48, 40),
            Rect::from_xywh(8.0, 7.0, 23.0, 14.0).unwrap(),
            4.0,
            1.0,
            original,
        ),
        (
            (48, 40),
            Rect::from_xywh(8.0, 7.0, 22.0, 15.0).unwrap(),
            4.0,
            1.0,
            original,
        ),
        ((48, 40), content(), 7.0, 1.0, original),
        ((48, 40), content(), 4.0, 2.0, original),
        (
            (48, 40),
            content(),
            4.0,
            1.0,
            Shadow {
                blur: 7.0,
                ..original
            },
        ),
        (
            (48, 40),
            content(),
            4.0,
            1.0,
            Shadow {
                offset_y: 5.0,
                ..original
            },
        ),
    ];
    for (size, content, radius, scale, changed) in cases {
        let mut cached = ShadowScratch::default();
        cached
            .prepare_mask((48, 40), self::content(), 4.0, 1.0, &original)
            .unwrap();
        let original_mask = cached.mask().to_vec();
        assert_eq!(
            cached.prepare_mask(size, content, radius, scale, &changed),
            Some(true)
        );
        assert_ne!(cached.mask(), original_mask);
        let mut fresh = ShadowScratch::default();
        fresh
            .prepare_mask(size, content, radius, scale, &changed)
            .unwrap();
        assert_eq!(cached.mask(), fresh.mask());
        assert_eq!(
            cached.prepare_mask(size, content, radius, scale, &changed),
            Some(false)
        );
    }
}

#[test]
fn color_change_reuses_geometry_but_updates_composited_pixels() {
    let mut scratch = ShadowScratch::default();
    let red = Shadow {
        color: Color::rgb(220, 0, 0),
        ..shadow()
    };
    let blue = Shadow {
        color: Color::rgb(0, 0, 220),
        ..red
    };
    assert_eq!(
        scratch.prepare_mask((48, 40), content(), 4.0, 1.0, &red),
        Some(true)
    );
    let coverage = scratch.mask().to_vec();
    assert_eq!(
        scratch.prepare_mask((48, 40), content(), 4.0, 1.0, &blue),
        Some(false)
    );
    assert_eq!(scratch.mask(), coverage);

    let mut first = Canvas::new(48, 40).unwrap();
    let mut second = Canvas::new(48, 40).unwrap();
    let mut expected = Canvas::new(48, 40).unwrap();
    red.paint(&mut first, content(), 4.0, 1.0);
    blue.paint(&mut second, content(), 4.0, 1.0);
    expected.blend_mask(0, 0, 48, 40, &coverage, blue.color);
    let first = first.into_pixmap();
    let second = second.into_pixmap();
    assert_ne!(first.pixels(), second.pixels());
    assert_eq!(second.pixels(), expected.into_pixmap().pixels());
    assert!(second.pixels().iter().any(|p| p.blue() > 0 && p.red() == 0));
}
