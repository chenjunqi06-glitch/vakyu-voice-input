//! 连续内存垂直模糊与原逐列实现的逐像素对照，包含小图、超大半径和三遍组合。

use super::blur::{horizontal, line, vertical};

fn column_reference(data: &mut [u8], width: usize, height: usize, radius: usize) {
    let mut column = vec![0; height];
    let mut blurred = vec![0; height];
    for col in 0..width {
        for row in 0..height {
            column[row] = data[row * width + col];
        }
        line(&column, &mut blurred, radius);
        for row in 0..height {
            data[row * width + col] = blurred[row];
        }
    }
}

#[test]
fn row_major_vertical_matches_column_reference_for_random_sizes_and_radii() {
    let mut state = 0x4d49_5241_u32;
    for (width, height) in [
        (1, 1),
        (1, 9),
        (11, 1),
        (2, 3),
        (7, 13),
        (43, 19),
        (127, 83),
    ] {
        for radius in [0, 1, 2, 5, 16, height + 3] {
            let mut source: Vec<u8> = (0..width * height)
                .map(|_| {
                    state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    (state >> 24) as u8
                })
                .collect();
            let mut reference = source.clone();
            let mut output = vec![0; source.len()];
            let mut sums = vec![0; width];
            let mut row = vec![0; width];
            for pass in 0..3 {
                horizontal(&mut source, &mut row, width, height, radius);
                horizontal(&mut reference, &mut row, width, height, radius);
                vertical(&source, &mut output, &mut sums, width, height, radius);
                column_reference(&mut reference, width, height, radius);
                assert_eq!(
                    output, reference,
                    "{width}x{height}, radius={radius}, pass={pass}"
                );
                std::mem::swap(&mut source, &mut output);
            }
        }
    }
}
