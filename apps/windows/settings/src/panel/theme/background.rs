//! 小幅无损位图提供左上到右下的真实渐变，避免 UI 框架只支持纯色画刷的限制。

use std::sync::{Arc, LazyLock};

use windows_reactor::{EncodedImage, Image, Stretch, View};

static BACKGROUND: LazyLock<EncodedImage> = LazyLock::new(|| {
    let side = 256_u32;
    let pixels = side * side * 4;
    let mut bitmap = Vec::with_capacity((54 + pixels) as usize);
    bitmap.extend_from_slice(b"BM");
    bitmap.extend_from_slice(&(54 + pixels).to_le_bytes());
    bitmap.extend_from_slice(&[0; 4]);
    bitmap.extend_from_slice(&54_u32.to_le_bytes());
    bitmap.extend_from_slice(&40_u32.to_le_bytes());
    bitmap.extend_from_slice(&side.to_le_bytes());
    // 负高度表示从上往下存放，渐变方向与界面坐标一致。
    bitmap.extend_from_slice(&(-(side as i32)).to_le_bytes());
    bitmap.extend_from_slice(&1_u16.to_le_bytes());
    bitmap.extend_from_slice(&32_u16.to_le_bytes());
    bitmap.extend_from_slice(&0_u32.to_le_bytes());
    bitmap.extend_from_slice(&pixels.to_le_bytes());
    bitmap.extend_from_slice(&[0; 16]);
    let start = [0x6D_i32, 0x35, 0xB5];
    let end = [0x35_i32, 0x20, 0x63];
    for y in 0..side {
        for x in 0..side {
            let color = std::array::from_fn::<_, 3, _>(|channel| {
                let delta = (end[channel] - start[channel]) * (x + y) as i32;
                (start[channel] + delta / (2 * (side - 1)) as i32) as u8
            });
            bitmap.extend_from_slice(&[color[2], color[1], color[0], 255]);
        }
    }
    EncodedImage::new(Arc::<[u8]>::from(bitmap))
});

pub(super) fn view() -> View {
    Image::new()
        .source_data(BACKGROUND.clone())
        .stretch(Stretch::Fill)
        .into()
}
