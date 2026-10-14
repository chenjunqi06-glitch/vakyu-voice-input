//! 无跨进程 COM 调用的前台与焦点快照，以及标准编辑控件的样式校验。

use std::mem::size_of;

use qingjian_platform::protocol::ScreenRect;
use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled;
use windows::Win32::UI::WindowsAndMessaging::{
    ES_PASSWORD, ES_READONLY, GUI_INMENUMODE, GUI_INMOVESIZE, GUI_POPUPMENUMODE,
    GUI_SYSTEMMENUMODE, GUITHREADINFO, GWL_STYLE, GetClassNameW, GetForegroundWindow,
    GetGUIThreadInfo, GetWindowLongW, GetWindowRect, GetWindowThreadProcessId, IsChild,
    SMTO_ABORTIFHUNG, SendMessageTimeoutW,
};

use super::desktop::validate_input_desktop;
use super::error::TargetError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct NativeFocus {
    pub(super) foreground: HWND,

    pub(super) focus: HWND,

    pub(super) foreground_pid: u32,

    pub(super) focus_pid: u32,

    pub(super) thread: u32,
}

impl NativeFocus {
    pub(super) fn capture() -> Result<(Self, ScreenRect), TargetError> {
        validate_input_desktop()?;
        let foreground = unsafe { GetForegroundWindow() };
        if foreground.is_invalid() {
            return Err(TargetError::NoEditableTarget);
        }
        let mut foreground_pid = 0;
        let thread = unsafe { GetWindowThreadProcessId(foreground, Some(&mut foreground_pid)) };
        let mut gui = GUITHREADINFO {
            cbSize: size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        unsafe { GetGUIThreadInfo(thread, &mut gui) }.map_err(|_| TargetError::NoEditableTarget)?;
        let forbidden = GUI_INMENUMODE | GUI_INMOVESIZE | GUI_POPUPMENUMODE | GUI_SYSTEMMENUMODE;
        let focus = gui.hwndFocus;
        if thread == 0
            || foreground_pid == 0
            || focus.is_invalid()
            || (gui.flags & forbidden).0 != 0
            || (focus != foreground && !unsafe { IsChild(foreground, focus) }.as_bool())
            || !unsafe { IsWindowEnabled(focus) }.as_bool()
        {
            return Err(TargetError::NoEditableTarget);
        }
        let mut focus_pid = 0;
        let focus_thread = unsafe { GetWindowThreadProcessId(focus, Some(&mut focus_pid)) };
        if focus_thread == 0 || focus_pid == 0 {
            return Err(TargetError::NoEditableTarget);
        }
        Ok((
            Self {
                foreground,
                focus,
                foreground_pid,
                focus_pid,
                thread: focus_thread,
            },
            caret_rect(&gui, focus),
        ))
    }

    pub(super) fn validate(&self) -> Result<(), TargetError> {
        match Self::capture() {
            Ok((current, _)) if current == *self => Ok(()),
            Err(TargetError::DesktopChanged) => Err(TargetError::DesktopChanged),
            _ => Err(TargetError::FocusChanged),
        }
    }

    pub(super) fn native_editable(&self) -> Result<bool, TargetError> {
        let mut class = [0u16; 256];
        let len = unsafe { GetClassNameW(self.focus, &mut class) };
        if len <= 0 || !is_native_edit(&String::from_utf16_lossy(&class[..len as usize])) {
            return Ok(false);
        }
        let style = unsafe { GetWindowLongW(self.focus, GWL_STYLE) };
        native_style_allows(style)?;
        let mut password_char = 0usize;
        // EM_GETPASSWORDCHAR 兼顾录音期间动态设置的掩码；超时不猜测安全属性。
        let queried = unsafe {
            SendMessageTimeoutW(
                self.focus,
                0x00D2,
                WPARAM(0),
                LPARAM(0),
                SMTO_ABORTIFHUNG,
                100,
                Some(&mut password_char),
            )
        };
        if queried.0 == 0 {
            return Err(TargetError::NoEditableTarget);
        }
        if password_char != 0 {
            return Err(TargetError::Password);
        }
        Ok(true)
    }
}

pub(super) fn is_native_edit(class: &str) -> bool {
    let class = class.to_ascii_lowercase();
    class == "edit" || class.starts_with("richedit") || class.starts_with("windowsforms10.edit.")
}

pub(super) fn native_style_allows(style: i32) -> Result<(), TargetError> {
    if style & ES_PASSWORD != 0 {
        Err(TargetError::Password)
    } else if style & ES_READONLY != 0 {
        Err(TargetError::ReadOnly)
    } else {
        Ok(())
    }
}

fn caret_rect(gui: &GUITHREADINFO, focus: HWND) -> ScreenRect {
    if !gui.hwndCaret.is_invalid() {
        let mut origin = POINT {
            x: gui.rcCaret.left,
            y: gui.rcCaret.top,
        };
        if unsafe { ClientToScreen(gui.hwndCaret, &mut origin) }.as_bool() {
            return ScreenRect {
                left: origin.x,
                top: origin.y,
                right: origin.x + (gui.rcCaret.right - gui.rcCaret.left).max(1),
                bottom: origin.y + (gui.rcCaret.bottom - gui.rcCaret.top).max(1),
            };
        }
    }
    let mut rect = RECT::default();
    let _ = unsafe { GetWindowRect(focus, &mut rect) };
    ScreenRect {
        left: rect.left,
        top: rect.top,
        right: rect.right,
        bottom: rect.bottom,
    }
}
