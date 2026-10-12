//! 线程退出前拆除低层输入钩子与定时器。

use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, UnhookWinEvent};
use windows::Win32::UI::WindowsAndMessaging::{HHOOK, KillTimer, UnhookWindowsHookEx};

#[derive(Default)]
pub(super) struct HookGuard {
    pub keyboard: Option<HHOOK>,

    pub mouse: Option<HHOOK>,

    pub timer: usize,

    pub foreground: Option<HWINEVENTHOOK>,

    pub focus: Option<HWINEVENTHOOK>,
}

impl Drop for HookGuard {
    fn drop(&mut self) {
        unsafe {
            if let Some(hook) = self.foreground {
                let _ = UnhookWinEvent(hook);
            }
            if let Some(hook) = self.focus {
                let _ = UnhookWinEvent(hook);
            }
            if self.timer != 0 {
                let _ = KillTimer(None, self.timer);
            }
            if let Some(hook) = self.mouse {
                let _ = UnhookWindowsHookEx(hook);
            }
            if let Some(hook) = self.keyboard {
                let _ = UnhookWindowsHookEx(hook);
            }
        }
    }
}
