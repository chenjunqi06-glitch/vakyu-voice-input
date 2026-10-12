//! 在调用方提供的工作区上运行一维盒式模糊，不逐帧分配临时行列。

pub(super) fn horizontal(
    data: &mut [u8],
    row_buf: &mut [u8],
    width: usize,
    height: usize,
    radius: usize,
) {
    for row in 0..height {
        let start = row * width;
        line(&data[start..start + width], row_buf, radius);
        data[start..start + width].copy_from_slice(row_buf);
    }
}

pub(super) fn vertical(
    source: &[u8],
    output: &mut [u8],
    sums: &mut [u32],
    width: usize,
    height: usize,
    radius: usize,
) {
    let window = (2 * radius + 1) as u32;
    sums.fill(0);
    for row in source.chunks_exact(width).take((radius + 1).min(height)) {
        for (sum, &value) in sums.iter_mut().zip(row) {
            *sum += u32::from(value);
        }
    }
    // 同时维护整行各列的滑动和，读写连续像素；输出完成前不覆盖输入。
    for (index, row) in output.chunks_exact_mut(width).enumerate().take(height) {
        for (out, &sum) in row.iter_mut().zip(sums.iter()) {
            *out = ((sum + window / 2) / window) as u8;
        }
        if index + radius + 1 < height {
            let start = (index + radius + 1) * width;
            for (sum, &value) in sums.iter_mut().zip(&source[start..start + width]) {
                *sum += u32::from(value);
            }
        }
        if index >= radius {
            let start = (index - radius) * width;
            for (sum, &value) in sums.iter_mut().zip(&source[start..start + width]) {
                *sum -= u32::from(value);
            }
        }
    }
}

/// 窗口 `2r + 1`，边界外当 0；滑动窗口 O(n)。
pub(super) fn line(src: &[u8], dst: &mut [u8], radius: usize) {
    let window = (2 * radius + 1) as u32;
    let mut sum: u32 = src.iter().take(radius + 1).map(|&v| u32::from(v)).sum();
    for i in 0..src.len() {
        dst[i] = ((sum + window / 2) / window) as u8;
        if i + radius + 1 < src.len() {
            sum += u32::from(src[i + radius + 1]);
        }
        if i >= radius {
            sum -= u32::from(src[i - radius]);
        }
    }
}
