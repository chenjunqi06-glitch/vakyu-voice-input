//! Round the settings window after WinUI creates it, including on Windows 10.

use std::{
    thread,
    time::{Duration, Instant},
};
use windows::Win32::{
    Foundation::{HWND, LPARAM},
    Graphics::Dwm::{DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmSetWindowAttribute},
    Graphics::Gdi::{CreateRoundRectRgn, DeleteObject, SetWindowRgn},
    System::Threading::GetCurrentProcessId,
    UI::HiDpi::GetDpiForWindow,
    UI::WindowsAndMessaging::{
        EnumWindows, GetWindowRect, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
        IsWindow, IsWindowVisible,
    },
};
use windows::core::BOOL;

pub(crate) fn round_settings_window_when_ready() {
    let process_id = unsafe { GetCurrentProcessId() };
    thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(30);
        let hwnd = loop {
            if let Some(hwnd) = settings_window(process_id) {
                break hwnd;
            }
            if Instant::now() >= deadline {
                return;
            }
            thread::sleep(Duration::from_millis(100));
        };

        if set_native_corners(hwnd) {
            return;
        }

        let mut previous_size = None;
        while unsafe { IsWindow(Some(hwnd)) }.as_bool() {
            let mut rect = Default::default();
            if unsafe { GetWindowRect(hwnd, &mut rect) }.is_ok() {
                let size = (rect.right - rect.left, rect.bottom - rect.top);
                if size.0 > 0 && size.1 > 0 && previous_size != Some(size) {
                    let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
                    let diameter = ((16 * dpi + 95) / 96) as i32;
                    if set_rounded_region(hwnd, size.0, size.1, diameter) {
                        previous_size = Some(size);
                    }
                }
            }
            thread::sleep(Duration::from_millis(80));
        }
    });
}

fn settings_window(process_id: u32) -> Option<HWND> {
    struct Search {
        process_id: u32,
        hwnd: Option<HWND>,
    }

    unsafe extern "system" fn find_window(hwnd: HWND, context: LPARAM) -> BOOL {
        let search = unsafe { &mut *(context.0 as *mut Search) };
        if !unsafe { IsWindowVisible(hwnd) }.as_bool() {
            return BOOL(1);
        }

        let mut owner_process_id = 0;
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut owner_process_id)) };
        if owner_process_id != search.process_id {
            return BOOL(1);
        }

        let length = unsafe { GetWindowTextLengthW(hwnd) };
        if length <= 0 {
            return BOOL(1);
        }
        let mut title = vec![0u16; length as usize + 1];
        let copied = unsafe { GetWindowTextW(hwnd, &mut title) };
        if String::from_utf16_lossy(&title[..copied as usize]) == "Vakyu 设置" {
            search.hwnd = Some(hwnd);
            BOOL(0)
        } else {
            BOOL(1)
        }
    }

    let mut search = Search {
        process_id,
        hwnd: None,
    };
    unsafe {
        let _ = EnumWindows(
            Some(find_window),
            LPARAM(&mut search as *mut Search as isize),
        );
    }
    search.hwnd
}

fn set_native_corners(hwnd: HWND) -> bool {
    let preference = DWMWCP_ROUND;
    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &preference as *const _ as *const _,
            std::mem::size_of_val(&preference) as u32,
        )
    }
    .is_ok()
}

fn set_rounded_region(hwnd: HWND, width: i32, height: i32, diameter: i32) -> bool {
    let region = unsafe { CreateRoundRectRgn(0, 0, width, height, diameter, diameter) };
    if region.is_invalid() {
        return false;
    }
    if unsafe { SetWindowRgn(hwnd, Some(region), true) } != 0 {
        true
    } else {
        let _ = unsafe { DeleteObject(region.into()) };
        false
    }
}
