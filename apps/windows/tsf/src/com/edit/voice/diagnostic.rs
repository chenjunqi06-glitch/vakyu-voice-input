//! 只记录上下文类型和范围长度，区分宿主实际正文与兼容层临时缓冲；不记录内容。

use windows::Win32::UI::TextServices::{ITfContext, ITfRange, ITfRangeACP, TF_SS_TRANSITORY};
use windows::core::Interface;

use crate::com::log::log;

pub(super) fn context(context: &ITfContext, phase: &str) {
    match unsafe { context.GetStatus() } {
        Ok(status) => log(&format!(
            "语音文档状态，阶段={phase}，transitory={}，static=0x{:X}，dynamic=0x{:X}",
            status.dwStaticFlags & TF_SS_TRANSITORY != 0,
            status.dwStaticFlags,
            status.dwDynamicFlags
        )),
        Err(error) => log(&format!(
            "语音文档状态读取失败，阶段={phase}，错误=0x{:08X}",
            error.code().0 as u32
        )),
    }
}

pub(super) fn inserted(range: &ITfRange, ec: u32, expected: usize) {
    let acp_length = range.cast::<ITfRangeACP>().ok().and_then(|acp| {
        let mut start = 0;
        let mut length = 0;
        unsafe { acp.GetExtent(&mut start, &mut length) }
            .ok()
            .map(|()| length)
    });
    // 通用范围不一定暴露 ACP；最多读到期望长度加一，只用于测长，缓冲不进入日志。
    let mut buffer = vec![0u16; expected.saturating_add(1).min(65_536)];
    let mut fetched = 0;
    let readable = unsafe { range.GetText(ec, 0, &mut buffer, &mut fetched) }.is_ok();
    log(&format!(
        "语音宿主返回范围，预期UTF16={expected}，ACP长度={acp_length:?}，可读={readable}，读取UTF16={fetched}，达到读取上限={}",
        fetched as usize == buffer.len()
    ));
}
