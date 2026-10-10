//! 在后台工作线程临时录制实体快捷键；离开设置窗口、取消或超时后立即卸载钩子。

mod keys;

use keys::Keys;
use std::cell::{Cell, RefCell};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

use qingjian_platform::PracticeShortcut;
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetForegroundWindow, GetMessageW, GetWindowThreadProcessId,
    HC_ACTION, HHOOK, KBDLLHOOKSTRUCT, KillTimer, LLKHF_EXTENDED, LLKHF_INJECTED,
    LLKHF_LOWER_IL_INJECTED, MSG, SetTimer, SetWindowsHookExW, TranslateMessage,
    UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
    WM_TIMER,
};
use windows_reactor::CancellationToken;

thread_local! {
    // 钩子只在安装线程的消息泵中回调；Box 固定地址，Capture::drop 先清除此指针。
    static ACTIVE: Cell<*const RefCell<Capture>> = const { Cell::new(std::ptr::null()) };
}

struct Capture {
    hook: Option<HHOOK>,

    timer: usize,

    cancel: CancellationToken,

    cancelled: Arc<AtomicBool>,

    started: Instant,

    keys: Keys,
}

impl Capture {
    fn stopped(&self) -> Option<String> {
        if self.cancel.is_cancelled() || self.cancelled.load(Ordering::Acquire) {
            Some("已取消快捷键录制".to_owned())
        } else if !settings_foreground() {
            Some("已离开设置窗口，快捷键录制已取消".to_owned())
        } else if self.started.elapsed() >= Duration::from_secs(30) {
            Some("快捷键录制已超时，请重试".to_owned())
        } else {
            None
        }
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        ACTIVE.with(|active| active.set(std::ptr::null()));
        unsafe {
            if let Some(hook) = self.hook.take() {
                let _ = UnhookWindowsHookEx(hook);
            }
            if self.timer != 0 {
                let _ = KillTimer(None, self.timer);
            }
        }
    }
}

/// 调用方通过 Reactor 后台任务调用；本线程只在录制期间运行消息泵。
pub(in crate::panel) fn record(
    cancel: &CancellationToken,
    cancelled: Arc<AtomicBool>,
) -> Result<PracticeShortcut, String> {
    if ACTIVE.with(|active| !active.get().is_null()) {
        return Err("已有快捷键正在录制".to_owned());
    }
    let mut pressed = [false; 256];
    for (vk, value) in pressed.iter_mut().enumerate().skip(8) {
        // 侧键足以表示修饰键；通用 VK_SHIFT / CONTROL / MENU 没有独立的抬起。
        if !(0x10..=0x12).contains(&vk) {
            *value = unsafe { GetAsyncKeyState(vk as i32) < 0 };
        }
    }
    let capture = Box::new(RefCell::new(Capture {
        hook: None,
        timer: 0,
        cancel: cancel.clone(),
        cancelled,
        started: Instant::now(),
        keys: Keys::new(pressed),
    }));
    if let Some(reason) = capture.borrow().stopped() {
        return Err(reason);
    }
    let module = unsafe { GetModuleHandleW(None) }
        .map_err(|error| format!("无法开始快捷键录制：{error}"))?;
    capture.borrow_mut().hook = Some(
        unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook), Some(module.into()), 0) }
            .map_err(|error| format!("无法监听快捷键：{error}"))?,
    );
    capture.borrow_mut().timer = unsafe { SetTimer(None, 0, 100, None) };
    if capture.borrow().timer == 0 {
        return Err("无法创建快捷键录制计时器".to_owned());
    }
    ACTIVE.with(|active| active.set(capture.as_ref()));
    let mut message = MSG::default();
    loop {
        {
            let mut capture = capture.borrow_mut();
            if let Some(reason) = capture.stopped() {
                return Err(reason);
            }
            if let Some(result) = capture.keys.take_result() {
                return result;
            }
        }
        let status = unsafe { GetMessageW(&mut message, None, 0, 0) }.0;
        if status <= 0 {
            return Err(if status == 0 {
                "已取消快捷键录制".to_owned()
            } else {
                "快捷键录制消息处理失败，请重试".to_owned()
            });
        }
        if message.message != WM_TIMER || message.wParam.0 != capture.borrow().timer {
            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
}

fn settings_foreground() -> bool {
    let foreground = unsafe { GetForegroundWindow() };
    if foreground.0.is_null() {
        return false;
    }
    let mut process = 0;
    unsafe { GetWindowThreadProcessId(foreground, Some(&mut process)) };
    process == std::process::id()
}

unsafe extern "system" fn keyboard_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code != HC_ACTION as i32 {
        return unsafe { CallNextHookEx(None, code, wparam, lparam) };
    }
    let message = wparam.0 as u32;
    if !matches!(message, WM_KEYDOWN | WM_KEYUP | WM_SYSKEYDOWN | WM_SYSKEYUP) {
        return unsafe { CallNextHookEx(None, code, wparam, lparam) };
    }
    let raw = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
    if raw.flags.0 & (LLKHF_INJECTED.0 | LLKHF_LOWER_IL_INJECTED.0) != 0 || raw.vkCode > 255 {
        return unsafe { CallNextHookEx(None, code, wparam, lparam) };
    }
    let extended = raw.flags.0 & LLKHF_EXTENDED.0 != 0;
    let vk = match raw.vkCode {
        0x10 => {
            if raw.scanCode == 0x36 {
                0xa1
            } else {
                0xa0
            }
        }
        0x11 => {
            if extended {
                0xa3
            } else {
                0xa2
            }
        }
        0x12 => {
            if extended {
                0xa5
            } else {
                0xa4
            }
        }
        vk => vk as u16,
    };
    let consumed = ACTIVE.with(|active| {
        // Box 由 record 持有，Drop 在释放内存前清指针；低层钩子在同一线程回调。
        let Some(capture) = (unsafe { active.get().as_ref() }) else {
            return false;
        };
        let Ok(mut capture) = capture.try_borrow_mut() else {
            return false;
        };
        if capture.stopped().is_some() {
            return false;
        }
        capture
            .keys
            .key(vk, matches!(message, WM_KEYDOWN | WM_SYSKEYDOWN))
    });
    if consumed {
        LRESULT(1)
    } else {
        unsafe { CallNextHookEx(None, code, wparam, lparam) }
    }
}
