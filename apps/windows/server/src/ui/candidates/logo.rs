//! 候选窗完整字母标志：保留原图比例按当前 DPI 缩放，并缓存最近尺寸。

use std::cell::RefCell;
use std::io::Cursor;
use std::sync::OnceLock;

use qingjian_render::Pixmap;
use tiny_skia::{FilterQuality, IntSize, PixmapPaint, Transform};

const MARK_PNG: &[u8] =
    include_bytes!("../../../../../../assets/icon/windows/miracle-candidate-mark.png");
const MARK_HEIGHT: f32 = 32.0;

struct RawMark {
    width: u32,

    height: u32,

    pixels: Vec<u8>,
}

static RAW_MARK: OnceLock<Option<RawMark>> = OnceLock::new();

thread_local! {
    static SIZED_MARK: RefCell<Option<((u32, u32), Pixmap)>> = const { RefCell::new(None) };
}

/// 横向完整标志保持比例；窄候选框内按可用宽度继续缩小。
pub(super) fn mark_size(scale: f32, available_width: i32) -> (u32, u32) {
    let Some(raw) = RAW_MARK.get_or_init(decode_mark).as_ref() else {
        return (1, 1);
    };
    let factor = (MARK_HEIGHT * scale / raw.height as f32)
        .min(available_width.max(1) as f32 / raw.width as f32);
    (
        (raw.width as f32 * factor).round().max(1.0) as u32,
        (raw.height as f32 * factor).round().max(1.0) as u32,
    )
}

/// 复用当前 DPI 下的缩放结果，避免每次按键都重新解码与缩放原图。
pub(super) fn with_sized_mark<T>(size: (u32, u32), draw: impl FnOnce(&Pixmap) -> T) -> Option<T> {
    SIZED_MARK.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache
            .as_ref()
            .is_none_or(|(cached_size, _)| *cached_size != size)
        {
            *cache = scale_mark(size).map(|mark| (size, mark));
        }
        cache.as_ref().map(|(_, mark)| draw(mark))
    })
}

fn scale_mark((width, height): (u32, u32)) -> Option<Pixmap> {
    let raw = RAW_MARK.get_or_init(decode_mark).as_ref()?;
    let size = IntSize::from_wh(raw.width, raw.height)?;
    let source = Pixmap::from_vec(raw.pixels.clone(), size)?;
    let mut scaled = Pixmap::new(width, height)?;
    let factor = (width as f32 / raw.width as f32).min(height as f32 / raw.height as f32);
    scaled.draw_pixmap(
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
    Some(scaled)
}

fn decode_mark() -> Option<RawMark> {
    let mut decoder = png::Decoder::new(Cursor::new(MARK_PNG));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let width = reader.info().width;
    let height = reader.info().height;
    let mut buffer = vec![0; reader.output_buffer_size()?];
    let output = reader.next_frame(&mut buffer).ok()?;
    if output.color_type != png::ColorType::Rgba {
        return None;
    }

    let mut pixels = buffer[..output.buffer_size()].to_vec();
    for pixel in pixels.as_chunks_mut::<4>().0 {
        let alpha = pixel[3] as u16;
        for channel in &mut pixel[..3] {
            *channel = ((*channel as u16 * alpha + 127) / 255) as u8;
        }
    }
    Some(RawMark {
        width,
        height,
        pixels,
    })
}

#[cfg(test)]
mod tests {
    use super::{MARK_HEIGHT, RAW_MARK, decode_mark, mark_size, with_sized_mark};

    #[test]
    fn candidate_mark_is_transparent_and_dpi_scaled() {
        let mark = RAW_MARK.get_or_init(decode_mark).as_ref().unwrap();
        assert_eq!((mark.width, mark.height), (832, 303));
        assert_eq!(mark.pixels[3], 0);
        assert!(
            mark.pixels[((mark.height / 2 * mark.width + mark.width / 2) * 4 + 3) as usize] >= 250
        );
        for scale in [1.0, 1.5, 2.0] {
            let size = mark_size(scale, 400);
            assert_eq!(size.1, (MARK_HEIGHT * scale) as u32);
            assert!(
                (size.0 as f32 / size.1 as f32 - mark.width as f32 / mark.height as f32).abs()
                    < 0.04
            );
            with_sized_mark(size, |scaled| {
                assert_eq!((scaled.width(), scaled.height()), size);
                for x in [size.0 / 8, size.0 * 7 / 8] {
                    assert!(
                        (0..size.1).any(|y| scaled.pixel(x, y).unwrap().alpha() > 100),
                        "左右字母必须保留"
                    );
                }
            })
            .unwrap();
        }
        let narrow = mark_size(2.0, 60);
        assert_eq!(narrow.0, 60);
        assert!(narrow.1 < 64);
    }
}
