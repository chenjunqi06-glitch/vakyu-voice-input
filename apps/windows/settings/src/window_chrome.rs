//! 设置窗口的无边框外壳：只在所属 UI 线程挂接，保留系统移动、缩放与窗口命令。

use std::cell::Cell;

use windows::Win32::{
    Foundation::{
        ERROR_SUCCESS, GetLastError, HWND, LPARAM, LRESULT, POINT, RECT, SetLastError, WPARAM,
    },
    Graphics::Dwm::{
        DWMNCRP_DISABLED, DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE, DWMWA_NCRENDERING_POLICY,
        DwmSetWindowAttribute,
    },
    Graphics::Gdi::{
        CreateRoundRectRgn, DeleteObject, GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO,
        MonitorFromWindow, SetWindowRgn,
    },
    System::Threading::{GetCurrentProcessId, GetCurrentThreadId},
    UI::HiDpi::GetDpiForWindow,
    UI::Input::KeyboardAndMouse::ReleaseCapture,
    UI::WindowsAndMessaging::{
        CallWindowProcW, DefWindowProcW, EnumWindows, GWL_EXSTYLE, GWL_STYLE, GWLP_WNDPROC,
        GetCursorPos, GetWindowLongPtrW, GetWindowRect, GetWindowTextLengthW, GetWindowTextW,
        GetWindowThreadProcessId, HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT, HTCAPTION, HTCLIENT,
        HTLEFT, HTRIGHT, HTTOP, HTTOPLEFT, HTTOPRIGHT, IsIconic, IsWindow, IsWindowVisible,
        IsZoomed, MINMAXINFO, NCCALCSIZE_PARAMS, PostMessageW, SC_MAXIMIZE, SC_MINIMIZE,
        SC_RESTORE, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
        SetWindowLongPtrW, SetWindowPos, WM_DPICHANGED, WM_GETMINMAXINFO, WM_NCACTIVATE,
        WM_NCCALCSIZE, WM_NCDESTROY, WM_NCHITTEST, WM_NCLBUTTONDBLCLK, WM_NCLBUTTONDOWN,
        WM_NCPAINT, WM_SIZE, WM_SYSCOMMAND, WNDPROC, WS_CAPTION, WS_EX_CLIENTEDGE,
        WS_EX_DLGMODALFRAME, WS_EX_STATICEDGE, WS_EX_WINDOWEDGE,
    },
};
use windows::core::BOOL;

thread_local! {
    static WINDOW: Cell<Option<HWND>> = const { Cell::new(None) };

    /// 按所属线程保存 UI 框架原窗口过程；设置程序只有一个主窗口。
    static ORIGINAL_PROC: Cell<Option<(HWND, WNDPROC)>> = const { Cell::new(None) };

    /// 标题高度与右侧按钮保留宽度均为 DIP，由同一份 XAML 布局传入。
    static HEADER: Cell<(i32, i32)> = const { Cell::new((48, 138)) };

    static REGION: Cell<Option<(i32, i32, u32, bool)>> = const { Cell::new(None) };
}

/// 在组件的 UI 消息中调用；窗口未创建时返回 false，可稍后重试。
pub(crate) fn attach(header_height: i32, buttons_width: i32) -> bool {
    let Some(hwnd) = settings_window() else {
        return false;
    };
    if unsafe { GetWindowThreadProcessId(hwnd, None) } != unsafe { GetCurrentThreadId() } {
        return false;
    }
    HEADER.set((header_height.clamp(24, 128), buttons_width.clamp(96, 320)));
    if WINDOW.get() == Some(hwnd) && original_proc(hwnd).is_some() {
        WINDOW.set(Some(hwnd));
        return true;
    }

    // 直接链接 user32 窗口过程，避免部分系统的 comctl32 缺少子类导出而无法启动。
    unsafe { SetLastError(ERROR_SUCCESS) };
    let previous_proc = unsafe { GetWindowLongPtrW(hwnd, GWLP_WNDPROC) };
    if previous_proc == 0 && unsafe { GetLastError() } != ERROR_SUCCESS {
        return false;
    }
    let previous_proc = if previous_proc == 0 {
        None
    } else {
        Some(unsafe {
            std::mem::transmute::<
                isize,
                unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT,
            >(previous_proc)
        })
    };
    if !set_window_proc(hwnd, Some(window_proc)) {
        return false;
    }
    ORIGINAL_PROC.set(Some((hwnd, previous_proc)));
    let original_style = unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) };
    let original_extended = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) };
    let style = original_style & !(WS_CAPTION.0 as isize);
    let extended = original_extended
        & !((WS_EX_CLIENTEDGE | WS_EX_DLGMODALFRAME | WS_EX_STATICEDGE | WS_EX_WINDOWEDGE).0
            as isize);
    if !set_style(hwnd, GWL_STYLE, style) || !set_style(hwnd, GWL_EXSTYLE, extended) {
        let _ = set_style(hwnd, GWL_STYLE, original_style);
        let _ = set_style(hwnd, GWL_EXSTYLE, original_extended);
        restore_window_proc(hwnd, previous_proc);
        ORIGINAL_PROC.set(None);
        return false;
    }
    WINDOW.set(Some(hwnd));
    // 保留 THICKFRAME / SYSMENU / MINIMIZEBOX / MAXIMIZEBOX 的行为，仅取消系统绘制。
    let policy = DWMNCRP_DISABLED;
    let color = DWMWA_COLOR_NONE;
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_NCRENDERING_POLICY,
            &policy as *const _ as *const _,
            std::mem::size_of_val(&policy) as u32,
        );
        // Windows 10 不支持此颜色属性；无边框布局与区域裁剪不依赖其成功。
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR,
            &color as *const _ as *const _,
            std::mem::size_of_val(&color) as u32,
        );
        let _ = SetWindowPos(
            hwnd,
            None,
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER,
        );
    }
    update_region(hwnd);
    crate::log::info("设置窗口已启用无系统边框样式");
    true
}

pub(crate) fn minimize() {
    post_system_command(SC_MINIMIZE);
}

pub(crate) fn maximize_or_restore() {
    if let Some(hwnd) = current_window() {
        let command = if unsafe { IsZoomed(hwnd) }.as_bool() {
            SC_RESTORE
        } else {
            SC_MAXIMIZE
        };
        let _ = unsafe {
            PostMessageW(
                Some(hwnd),
                WM_SYSCOMMAND,
                WPARAM(command as usize),
                LPARAM(0),
            )
        };
    }
}

/// 可供标题区的按下事件调用；常规鼠标拖动已由 WM_NCHITTEST 提供。
pub(crate) fn begin_drag() {
    if let Some(hwnd) = current_window() {
        let mut cursor = POINT::default();
        if unsafe { GetCursorPos(&mut cursor) }.is_ok() {
            let position = ((cursor.x as u16 as u32) | ((cursor.y as u16 as u32) << 16)) as isize;
            unsafe {
                let _ = ReleaseCapture();
                // 等本轮组件更新结束后再进入系统拖动循环，避免重入 WinUI 布局。
                let _ = PostMessageW(
                    Some(hwnd),
                    WM_NCLBUTTONDOWN,
                    WPARAM(HTCAPTION as usize),
                    LPARAM(position),
                );
            }
        }
    }
}

fn current_window() -> Option<HWND> {
    WINDOW
        .get()
        .filter(|hwnd| unsafe { IsWindow(Some(*hwnd)) }.as_bool())
        .or_else(settings_window)
}

fn post_system_command(command: u32) {
    if let Some(hwnd) = current_window() {
        let _ = unsafe {
            PostMessageW(
                Some(hwnd),
                WM_SYSCOMMAND,
                WPARAM(command as usize),
                LPARAM(0),
            )
        };
    }
}

fn set_style(
    hwnd: HWND,
    index: windows::Win32::UI::WindowsAndMessaging::WINDOW_LONG_PTR_INDEX,
    value: isize,
) -> bool {
    unsafe {
        SetLastError(ERROR_SUCCESS);
        SetWindowLongPtrW(hwnd, index, value) != 0 || GetLastError() == ERROR_SUCCESS
    }
}

fn set_window_proc(hwnd: HWND, proc: WNDPROC) -> bool {
    unsafe {
        SetLastError(ERROR_SUCCESS);
        let previous = SetWindowLongPtrW(
            hwnd,
            GWLP_WNDPROC,
            proc.map(|proc| proc as usize as isize).unwrap_or(0),
        );
        previous != 0 || GetLastError() == ERROR_SUCCESS
    }
}

fn restore_window_proc(hwnd: HWND, proc: WNDPROC) {
    let _ = set_window_proc(hwnd, proc);
}

fn original_proc(hwnd: HWND) -> WNDPROC {
    ORIGINAL_PROC
        .get()
        .and_then(|(stored_hwnd, proc)| (stored_hwnd == hwnd).then_some(proc))
        .flatten()
}

unsafe fn call_original(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if let Some(proc) = original_proc(hwnd) {
        unsafe { CallWindowProcW(Some(proc), hwnd, message, wparam, lparam) }
    } else {
        unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
    }
}

fn settings_window() -> Option<HWND> {
    unsafe extern "system" fn find_window(hwnd: HWND, context: LPARAM) -> BOOL {
        let search = unsafe { &mut *(context.0 as *mut (u32, Option<HWND>)) };
        let mut owner = 0;
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut owner)) };
        if owner != search.0 || !unsafe { IsWindowVisible(hwnd) }.as_bool() {
            return BOOL(1);
        }
        let length = unsafe { GetWindowTextLengthW(hwnd) };
        if !(1..=256).contains(&length) {
            return BOOL(1);
        }
        let mut title = vec![0u16; length as usize + 1];
        let copied = unsafe { GetWindowTextW(hwnd, &mut title) };
        if String::from_utf16_lossy(&title[..copied as usize]) == "Vakyu 设置" {
            search.1 = Some(hwnd);
            BOOL(0)
        } else {
            BOOL(1)
        }
    }
    let mut search = (unsafe { GetCurrentProcessId() }, None);
    unsafe {
        let _ = EnumWindows(Some(find_window), LPARAM(&mut search as *mut _ as isize));
    }
    search.1
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_NCCALCSIZE => {
            if lparam.0 != 0 && unsafe { IsZoomed(hwnd) }.as_bool() {
                let rect = if wparam.0 != 0 {
                    &mut unsafe { &mut *(lparam.0 as *mut NCCALCSIZE_PARAMS) }.rgrc[0]
                } else {
                    unsafe { &mut *(lparam.0 as *mut RECT) }
                };
                if let Some(monitor) = monitor_info(hwnd) {
                    *rect = monitor.rcWork;
                }
            }
            LRESULT(0)
        }
        WM_NCHITTEST => hit_test(hwnd, lparam),
        WM_NCPAINT => LRESULT(0),
        WM_NCACTIVATE => unsafe { call_original(hwnd, message, wparam, LPARAM(-1)) },
        WM_GETMINMAXINFO => {
            let result = unsafe { call_original(hwnd, message, wparam, lparam) };
            if lparam.0 != 0
                && let Some(monitor) = monitor_info(hwnd)
            {
                let limits = unsafe { &mut *(lparam.0 as *mut MINMAXINFO) };
                limits.ptMaxPosition = POINT {
                    x: monitor.rcWork.left - monitor.rcMonitor.left,
                    y: monitor.rcWork.top - monitor.rcMonitor.top,
                };
                limits.ptMaxSize = POINT {
                    x: monitor.rcWork.right - monitor.rcWork.left,
                    y: monitor.rcWork.bottom - monitor.rcWork.top,
                };
            }
            result
        }
        WM_NCLBUTTONDBLCLK if wparam.0 == HTCAPTION as usize => {
            maximize_or_restore();
            LRESULT(0)
        }
        WM_SIZE | WM_DPICHANGED => {
            let result = unsafe { call_original(hwnd, message, wparam, lparam) };
            update_region(hwnd);
            result
        }
        WM_NCDESTROY => {
            let previous_proc = original_proc(hwnd);
            restore_window_proc(hwnd, previous_proc);
            WINDOW.set(None);
            ORIGINAL_PROC.set(None);
            REGION.set(None);
            if let Some(proc) = previous_proc {
                unsafe { CallWindowProcW(Some(proc), hwnd, message, wparam, lparam) }
            } else {
                unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
            }
        }
        _ => unsafe { call_original(hwnd, message, wparam, lparam) },
    }
}

fn hit_test(hwnd: HWND, position: LPARAM) -> LRESULT {
    let mut rect = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut rect) }.is_err() {
        return LRESULT(HTCLIENT as isize);
    }
    let x = (position.0 as u16 as i16) as i32 - rect.left;
    let y = ((position.0 >> 16) as u16 as i16) as i32 - rect.top;
    let size = (rect.right - rect.left, rect.bottom - rect.top);
    let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
    LRESULT(hit_region(
        x,
        y,
        size,
        dpi,
        unsafe { IsZoomed(hwnd) }.as_bool(),
        HEADER.get(),
    ) as isize)
}

fn hit_region(
    x: i32,
    y: i32,
    size: (i32, i32),
    dpi: u32,
    maximized: bool,
    header: (i32, i32),
) -> u32 {
    let pixels = |dip: i32| ((dip as i64 * dpi as i64 + 95) / 96) as i32;
    if !maximized {
        let edge = pixels(6);
        let left = x < edge;
        let right = x >= size.0 - edge;
        let top = y < edge;
        let bottom = y >= size.1 - edge;
        match (left, right, top, bottom) {
            (true, _, true, _) => return HTTOPLEFT,
            (_, true, true, _) => return HTTOPRIGHT,
            (true, _, _, true) => return HTBOTTOMLEFT,
            (_, true, _, true) => return HTBOTTOMRIGHT,
            (true, _, _, _) => return HTLEFT,
            (_, true, _, _) => return HTRIGHT,
            (_, _, true, _) => return HTTOP,
            (_, _, _, true) => return HTBOTTOM,
            _ => {}
        }
    }
    if y >= 0 && y < pixels(header.0) && x >= 0 && x < size.0 - pixels(header.1) {
        HTCAPTION
    } else {
        HTCLIENT
    }
}

fn monitor_info(hwnd: HWND) -> Option<MONITORINFO> {
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    unsafe { GetMonitorInfoW(monitor, &mut info) }
        .as_bool()
        .then_some(info)
}

fn update_region(hwnd: HWND) {
    if unsafe { IsIconic(hwnd) }.as_bool() {
        return;
    }
    let mut rect = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut rect) }.is_err() {
        return;
    }
    let (width, height) = (rect.right - rect.left, rect.bottom - rect.top);
    let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
    let maximized = unsafe { IsZoomed(hwnd) }.as_bool();
    let state = (width, height, dpi, maximized);
    if width <= 0 || height <= 0 || REGION.get() == Some(state) {
        return;
    }
    // SetWindowRgn 可引发嵌套尺寸消息，先记住尺寸以免递归更新。
    REGION.set(Some(state));
    if maximized {
        let _ = unsafe { SetWindowRgn(hwnd, None, true) };
        return;
    }
    let diameter = ((16 * dpi + 95) / 96) as i32;
    let region = unsafe { CreateRoundRectRgn(0, 0, width, height, diameter, diameter) };
    if region.is_invalid() {
        REGION.set(None);
    } else if unsafe { SetWindowRgn(hwnd, Some(region), true) } == 0 {
        REGION.set(None);
        let _ = unsafe { DeleteObject(region.into()) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_drag_does_not_take_over_caption_buttons_or_page() {
        assert_eq!(
            hit_region(200, 24, (1060, 780), 96, false, (48, 156)),
            HTCAPTION
        );
        assert_eq!(
            hit_region(980, 24, (1060, 780), 96, false, (48, 156)),
            HTCLIENT
        );
        assert_eq!(
            hit_region(200, 90, (1060, 780), 96, false, (48, 156)),
            HTCLIENT
        );
    }

    #[test]
    fn all_edges_resize_and_maximized_window_only_drags() {
        for (point, expected) in [
            ((1, 1), HTTOPLEFT),
            ((500, 1), HTTOP),
            ((1059, 1), HTTOPRIGHT),
            ((1, 400), HTLEFT),
            ((1059, 400), HTRIGHT),
            ((1, 779), HTBOTTOMLEFT),
            ((500, 779), HTBOTTOM),
            ((1059, 779), HTBOTTOMRIGHT),
        ] {
            assert_eq!(
                hit_region(point.0, point.1, (1060, 780), 96, false, (48, 156)),
                expected
            );
        }
        assert_eq!(
            hit_region(1, 1, (1060, 780), 96, true, (48, 156)),
            HTCAPTION
        );
        assert_eq!(
            hit_region(1, 779, (1060, 780), 96, true, (48, 156)),
            HTCLIENT
        );
    }

    #[test]
    fn header_and_buttons_scale_together_with_dpi() {
        assert_eq!(
            hit_region(300, 66, (1590, 1170), 144, false, (48, 156)),
            HTCAPTION
        );
        assert_eq!(
            hit_region(1500, 36, (1590, 1170), 144, false, (48, 156)),
            HTCLIENT
        );
        assert_eq!(
            hit_region(300, 73, (1590, 1170), 144, false, (48, 156)),
            HTCLIENT
        );
    }
}
