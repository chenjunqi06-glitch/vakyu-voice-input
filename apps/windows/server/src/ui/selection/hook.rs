//! 划词翻译的全局鼠标低层钩子：只判断「刚刚像不像在选文字」（I 形光标下拖选或双击），把坐标投给 UI 线程；
//! 不读取任何文字，也不吞掉鼠标事件。独立线程跑自己的消息循环，关闭时投 `WM_QUIT`。

use std::cell::Cell;
use std::sync::mpsc;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    CURSORINFO, CallNextHookEx, DispatchMessageW, GetCursorInfo, GetDoubleClickTime, GetMessageW,
    HCURSOR, IDC_IBEAM, LLMHF_INJECTED, LoadCursorW, MSG, MSLLHOOKSTRUCT, PostMessageW,
    PostThreadMessageW, SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, WH_MOUSE_LL,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_QUIT, WM_RBUTTONDOWN,
};

use super::{MSG_CANDIDATE, MSG_PRESS};

/// 拖动超过这么多物理像素才算拖选，避免普通点击误弹图标。
const DRAG_THRESHOLD: i32 = 6;

/// 双击两次松开的最大距离。
const DOUBLE_CLICK_SLOP: i32 = 4;

thread_local! {
    /// 接收消息的图标窗口（`HWND` 不能跨线程，存成整数）。
    static TARGET: Cell<isize> = const { Cell::new(0) };

    /// 本次左键按下的位置，以及按下时是不是 I 形光标。
    static DOWN: Cell<Option<(POINT, bool)>> = const { Cell::new(None) };

    /// 上一次左键松开的位置与时间，用来认双击。
    static LAST_UP: Cell<Option<(POINT, u32)>> = const { Cell::new(None) };

    /// 系统共享的 I 形光标句柄。
    static IBEAM: Cell<isize> = const { Cell::new(0) };
}

/// 钩子线程的句柄；丢弃时让线程退出并卸下钩子。
pub(super) struct SelectionHook {
    thread_id: u32,
}

impl SelectionHook {
    /// 起钩子线程，消息投给 `target`；装不上钩子返回 `None`。
    pub(super) fn start(target: HWND) -> Option<Self> {
        let target = target.0 as isize;
        let (ready_tx, ready_rx) = mpsc::channel::<Option<u32>>();
        std::thread::Builder::new()
            .name("vakyu-selection-hook".to_owned())
            .spawn(move || run(target, &ready_tx))
            .ok()?;
        let thread_id = ready_rx.recv().ok().flatten()?;
        Some(Self { thread_id })
    }
}

impl Drop for SelectionHook {
    fn drop(&mut self) {
        let _ = unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) };
    }
}

fn run(target: isize, ready: &mpsc::Sender<Option<u32>>) {
    TARGET.with(|cell| cell.set(target));
    let ibeam = unsafe { LoadCursorW(None, IDC_IBEAM) }.unwrap_or_default();
    IBEAM.with(|cell| cell.set(ibeam.0 as isize));
    let module = match unsafe { GetModuleHandleW(None) } {
        Ok(module) => module,
        Err(error) => {
            tracing::warn!(%error, "划词翻译：取不到模块句柄");
            let _ = ready.send(None);
            return;
        }
    };
    let hook =
        match unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook), Some(module.into()), 0) } {
            Ok(hook) => hook,
            Err(error) => {
                tracing::warn!(%error, "划词翻译：鼠标钩子安装失败");
                let _ = ready.send(None);
                return;
            }
        };
    if ready.send(Some(unsafe { GetCurrentThreadId() })).is_err() {
        let _ = unsafe { UnhookWindowsHookEx(hook) };
        return;
    }
    let mut msg = MSG::default();
    loop {
        let got = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        if got.0 <= 0 {
            break;
        }
        unsafe {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    let _ = unsafe { UnhookWindowsHookEx(hook) };
}

/// 钩子回调要快：只记坐标、比较光标，投一条消息就返回。
unsafe extern "system" fn mouse_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let raw = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
        // 自己或其他程序模拟的点击（包括划词翻译发的 Ctrl+C 之外的模拟输入）不算用户选字。
        if raw.flags & LLMHF_INJECTED == 0 {
            on_mouse(wparam.0 as u32, raw.pt, raw.time);
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

fn on_mouse(message: u32, point: POINT, time: u32) {
    match message {
        WM_LBUTTONDOWN => {
            post(MSG_PRESS, point);
            DOWN.with(|cell| cell.set(Some((point, cursor_is_ibeam()))));
        }
        WM_RBUTTONDOWN | WM_MBUTTONDOWN => post(MSG_PRESS, point),
        WM_LBUTTONUP => {
            let Some((start, ibeam_at_down)) = DOWN.with(Cell::take) else {
                return;
            };
            let dragged = (point.x - start.x).abs() >= DRAG_THRESHOLD
                || (point.y - start.y).abs() >= DRAG_THRESHOLD;
            let double = LAST_UP.with(Cell::get).is_some_and(|(last, at)| {
                time.wrapping_sub(at) <= unsafe { GetDoubleClickTime() }
                    && (point.x - last.x).abs() <= DOUBLE_CLICK_SLOP
                    && (point.y - last.y).abs() <= DOUBLE_CLICK_SLOP
            });
            LAST_UP.with(|cell| cell.set(Some((point, time))));
            if (dragged || double) && (ibeam_at_down || cursor_is_ibeam()) {
                post(MSG_CANDIDATE, point);
            }
        }
        _ => {}
    }
}

fn cursor_is_ibeam() -> bool {
    let mut info = CURSORINFO {
        cbSize: size_of::<CURSORINFO>() as u32,
        ..Default::default()
    };
    if unsafe { GetCursorInfo(&mut info) }.is_err() {
        return false;
    }
    let ibeam = IBEAM.with(Cell::get);
    ibeam != 0 && info.hCursor == HCURSOR(ibeam as *mut core::ffi::c_void)
}

fn post(message: u32, point: POINT) {
    let target = TARGET.with(Cell::get);
    if target == 0 {
        return;
    }
    let hwnd = HWND(target as *mut core::ffi::c_void);
    let _ = unsafe {
        PostMessageW(
            Some(hwnd),
            message,
            WPARAM(point.x as isize as usize),
            LPARAM(point.y as isize),
        )
    };
}
