//! 借用剪贴板读取选区：先完整备份剪贴板，模拟一次 Ctrl+C，读到文字后把原内容放回去。
//! 只备份基于全局内存的格式（文字、HTML、RTF、图片 DIB、文件列表等）；位图句柄这类 GDI 格式由系统从 DIB 自动合成。

use std::time::Duration;

use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, EnumClipboardFormats, GetClipboardData,
    GetClipboardSequenceNumber, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{
    GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput,
    VIRTUAL_KEY, VK_CONTROL,
};

/// `CF_UNICODETEXT`。
const UNICODE_TEXT: u32 = 13;

/// 备份上限：超过就不借用剪贴板，免得复制大图后读选区卡住或丢内容。
const MAX_SNAPSHOT_BYTES: usize = 64 << 20;

/// 备份下来的剪贴板：格式号与内容字节，按原顺序放回。
pub(super) struct Snapshot {
    items: Vec<(u32, Vec<u8>)>,
}

/// 剪贴板序号；应用写入剪贴板后会变。
pub(super) fn sequence() -> u32 {
    unsafe { GetClipboardSequenceNumber() }
}

/// 备份当前剪贴板；打不开或内容过大返回 `None`。
pub(super) fn snapshot(owner: HWND) -> Option<Snapshot> {
    let _open = Open::new(owner)?;
    let mut items = Vec::new();
    let mut total = 0usize;
    let mut format = 0u32;
    loop {
        format = unsafe { EnumClipboardFormats(format) };
        if format == 0 {
            break;
        }
        if !memory_format(format) {
            continue;
        }
        let Ok(handle) = (unsafe { GetClipboardData(format) }) else {
            continue;
        };
        if let Some(bytes) = read_global(HGLOBAL(handle.0)) {
            total += bytes.len();
            if total > MAX_SNAPSHOT_BYTES {
                return None;
            }
            items.push((format, bytes));
        }
    }
    Some(Snapshot { items })
}

/// 把备份放回剪贴板（原来是空的就清空）。
pub(super) fn restore(owner: HWND, snapshot: &Snapshot) {
    let Some(_open) = Open::new(owner) else {
        tracing::warn!("划词翻译：剪贴板被占用，原内容未能放回");
        return;
    };
    let _ = unsafe { EmptyClipboard() };
    for (format, bytes) in &snapshot.items {
        if let Some(memory) = global_from(bytes)
            && unsafe { SetClipboardData(*format, Some(HANDLE(memory.0))) }.is_err()
        {
            let _ = unsafe { GlobalFree(Some(memory)) };
        }
    }
}

/// 读剪贴板里的 Unicode 文字。
pub(super) fn read_text(owner: HWND) -> Option<String> {
    let _open = Open::new(owner)?;
    let handle = unsafe { GetClipboardData(UNICODE_TEXT) }.ok()?;
    let bytes = read_global(HGLOBAL(handle.0))?;
    let units: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .take_while(|unit| *unit != 0)
        .collect();
    Some(String::from_utf16_lossy(&units))
}

/// 「复制译文」：把文字放进剪贴板。
pub(super) fn write_text(owner: HWND, text: &str) -> bool {
    let Some(_open) = Open::new(owner) else {
        return false;
    };
    let mut bytes: Vec<u8> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
    bytes.extend_from_slice(&[0, 0]);
    let _ = unsafe { EmptyClipboard() };
    let Some(memory) = global_from(&bytes) else {
        return false;
    };
    if unsafe { SetClipboardData(UNICODE_TEXT, Some(HANDLE(memory.0))) }.is_err() {
        let _ = unsafe { GlobalFree(Some(memory)) };
        return false;
    }
    true
}

/// 向前台软件发一次 Ctrl+C；图标和小窗都不抢焦点，所以按键落在原软件的选区上。
pub(super) fn send_copy() -> bool {
    let key = |vk: VIRTUAL_KEY, up: bool| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: if up {
                    KEYEVENTF_KEYUP
                } else {
                    KEYBD_EVENT_FLAGS(0)
                },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let letter_c = VIRTUAL_KEY(0x43);
    let inputs = [
        key(VK_CONTROL, false),
        key(letter_c, false),
        key(letter_c, true),
        key(VK_CONTROL, true),
    ];
    let sent = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
    sent as usize == inputs.len()
}

/// 位图、调色板、图元文件与私有 GDI 格式不是全局内存，不能按字节备份。
fn memory_format(format: u32) -> bool {
    !matches!(
        format,
        2 | 3 | 9 | 14 | 0x80 | 0x82 | 0x83 | 0x8E | 0x300..=0x3FF
    )
}

fn read_global(memory: HGLOBAL) -> Option<Vec<u8>> {
    let size = unsafe { GlobalSize(memory) };
    if size == 0 || size > MAX_SNAPSHOT_BYTES {
        return None;
    }
    let pointer = unsafe { GlobalLock(memory) };
    if pointer.is_null() {
        return None;
    }
    let bytes = unsafe { std::slice::from_raw_parts(pointer as *const u8, size) }.to_vec();
    let _ = unsafe { GlobalUnlock(memory) };
    Some(bytes)
}

fn global_from(bytes: &[u8]) -> Option<HGLOBAL> {
    let memory = unsafe { GlobalAlloc(GMEM_MOVEABLE, bytes.len().max(1)) }.ok()?;
    let pointer = unsafe { GlobalLock(memory) };
    if pointer.is_null() {
        let _ = unsafe { GlobalFree(Some(memory)) };
        return None;
    }
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), pointer as *mut u8, bytes.len()) };
    let _ = unsafe { GlobalUnlock(memory) };
    Some(memory)
}

/// 打开的剪贴板；别的程序正占着时稍等重试几次，离开作用域自动关闭。
struct Open;

impl Open {
    fn new(owner: HWND) -> Option<Self> {
        for _ in 0..5 {
            if unsafe { OpenClipboard(Some(owner)) }.is_ok() {
                return Some(Self);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        None
    }
}

impl Drop for Open {
    fn drop(&mut self) {
        let _ = unsafe { CloseClipboard() };
    }
}

#[cfg(test)]
mod tests {
    use super::memory_format;

    #[test]
    fn gdi_formats_are_not_copied_as_bytes() {
        assert!(memory_format(13), "Unicode 文字要备份");
        assert!(memory_format(8), "DIB 图片要备份");
        assert!(memory_format(15), "文件列表要备份");
        assert!(memory_format(0xC0FF), "注册格式（HTML 等）要备份");
        for gdi in [2, 3, 9, 14, 0x80, 0x82, 0x83, 0x8E, 0x300, 0x3FF] {
            assert!(!memory_format(gdi));
        }
    }
}
