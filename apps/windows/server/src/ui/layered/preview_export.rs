//! 测试专用的一次性 PNG 导出；只读取本线程刚合成的位图，不读取屏幕或其他窗口。

use std::cell::RefCell;
use std::path::PathBuf;

use qingjian_render::Pixmap;
use windows::Win32::Foundation::{E_FAIL, E_INVALIDARG};
use windows::core::{Error, Result};

thread_local! {
    static OUTPUT: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

/// 必须由测试显式给出绝对路径，只保存下一帧；生产构建中没有此入口。
pub(in crate::ui) fn export_next_frame(path: PathBuf) -> Result<()> {
    if !path.is_absolute() || path.extension().is_none_or(|extension| extension != "png") {
        return Err(Error::new(E_INVALIDARG, "预览导出需要 PNG 文件的绝对路径"));
    }
    OUTPUT.with_borrow_mut(|output| *output = Some(path));
    Ok(())
}

pub(super) fn export_once(pixels: &[u8], width: i32, height: i32) -> Result<()> {
    let Some(path) = OUTPUT.with_borrow_mut(Option::take) else {
        return Ok(());
    };
    let mut pixmap = Pixmap::new(width as u32, height as u32)
        .ok_or_else(|| Error::new(E_FAIL, "无法分配预览位图"))?;
    if pixmap.data().len() != pixels.len() {
        return Err(Error::new(E_INVALIDARG, "预览位图尺寸与像素数据不一致"));
    }
    // 本身就是预乘 alpha，仅交换 BGRA/RGBA 通道；PNG 编码由 Pixmap 处理。
    for (target, source) in pixmap
        .data_mut()
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(pixels.as_chunks::<4>().0)
    {
        target.copy_from_slice(&[source[2], source[1], source[0], source[3]]);
    }
    pixmap
        .save_png(path)
        .map_err(|error| Error::new(E_FAIL, format!("保存预览 PNG 失败：{error}")))
}
