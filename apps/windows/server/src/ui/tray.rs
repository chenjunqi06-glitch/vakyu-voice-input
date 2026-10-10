//! Server 独立托盘图标：即使前台程序没有加载 TSF，也能打开设置和切换主要开关。

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use windows::Win32::Foundation::{E_FAIL, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::UI::Shell::{
    NIF_GUID, NIF_ICON, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_SETVERSION,
    NOTIFYICON_VERSION_4, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CS_DBLCLKS, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu,
    DestroyWindow, GetCursorPos, KillTimer, LoadIconW, MENU_ITEM_FLAGS, MF_CHECKED, MF_GRAYED,
    MF_SEPARATOR, MF_STRING, PostMessageW, RegisterWindowMessageW, SetForegroundWindow, SetTimer,
    TPM_BOTTOMALIGN, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenuEx, WM_CONTEXTMENU, WM_DESTROY,
    WM_ENDSESSION, WM_LBUTTONDBLCLK, WM_LBUTTONUP, WM_NULL, WM_RBUTTONUP, WM_TIMER, WNDCLASSEXW,
    WS_EX_TOOLWINDOW, WS_POPUP,
};
use windows::core::{Error, GUID, HSTRING, PCWSTR, Result, w};

use super::StatusEvents;
use super::window_class::WindowClass;
use crate::dispatch::{StatusEvent, TrayAction, TrayState};

const CLASS_NAME: PCWSTR = w!("MiracleTrayIcon");
const TRAY_MESSAGE: u32 = 0x8000 + 29;
const ICON_ID: u32 = 1;
// HWND 每次重启都会变化；固定 GUID 才能替换上次异常退出遗留的通知区图标。
const ICON_GUID: GUID = GUID::from_u128(0x424769d0_64c2_44d6_93e4_0a93b635e12b);
const RETRY_TIMER: usize = 1;
const RETRY_INTERVAL_MS: u32 = 1_500;
const MAX_RETRIES: u32 = 40;
const ID_VOICE: usize = 1;
const ID_PRACTICE: usize = 2;
const ID_DEMO: usize = 3;
const ID_CLOUD: usize = 4;
const ID_STATUS: usize = 5;
const ID_PUNCTUATION: usize = 6;
const ID_SETTINGS: usize = 7;
const ID_DIRECTION: usize = 8;

static CLASS: WindowClass = WindowClass::new();

thread_local! {
    static TRAYS: RefCell<HashMap<isize, Weak<TrayIcon>>> = RefCell::new(HashMap::new());
    static TASKBAR_CREATED: Cell<u32> = const { Cell::new(0) };
}

pub(super) struct TrayIcon {
    hwnd: HWND,

    state: Cell<TrayState>,

    events: StatusEvents,

    registered: Cell<bool>,

    retries: Cell<u32>,
}

impl TrayIcon {
    pub(super) fn new(events: StatusEvents) -> Result<Rc<Self>> {
        CLASS.ensure(|| WNDCLASSEXW {
            lpfnWndProc: Some(wndproc),
            hInstance: super::module_handle(),
            lpszClassName: CLASS_NAME,
            style: CS_DBLCLKS,
            ..Default::default()
        })?;
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW,
                CLASS_NAME,
                w!("Vakyu"),
                WS_POPUP,
                0,
                0,
                0,
                0,
                None,
                None,
                Some(super::module_handle()),
                None,
            )?
        };
        let tray = Rc::new(Self {
            hwnd,
            state: Cell::new(TrayState::default()),
            events,
            registered: Cell::new(false),
            retries: Cell::new(0),
        });
        TRAYS.with(|trays| {
            trays
                .borrow_mut()
                .insert(hwnd.0 as isize, Rc::downgrade(&tray));
        });
        tray.restore();
        Ok(tray)
    }

    pub(super) fn configure(&self, state: TrayState) {
        self.state.set(state);
    }

    fn icon_data(&self) -> NOTIFYICONDATAW {
        let mut tip = [0u16; 128];
        let text: Vec<u16> = "Vakyu 输入法".encode_utf16().chain([0]).collect();
        let tip_len = text.len().min(tip.len());
        tip[..tip_len].copy_from_slice(&text[..tip_len]);
        // Win32 MAKEINTRESOURCEW 用低位整数表示资源 ID，不是可解引用的地址。
        #[allow(clippy::manual_dangling_ptr)]
        let icon = unsafe {
            LoadIconW(Some(super::module_handle()), PCWSTR(1 as *const u16)).unwrap_or_default()
        };
        NOTIFYICONDATAW {
            uFlags: NIF_GUID | NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP,
            uCallbackMessage: TRAY_MESSAGE,
            hIcon: icon,
            szTip: tip,
            ..icon_identity(self.hwnd)
        }
    }

    fn install(&self) -> Result<()> {
        let data = self.icon_data();
        // 后台已预留管道保证单实例；启动及 Explorer 重建时清理同一身份再注册。
        let _ = unsafe { Shell_NotifyIconW(NIM_DELETE, &icon_identity(self.hwnd)) };
        if !unsafe { Shell_NotifyIconW(NIM_ADD, &data) }.as_bool() {
            return Err(Error::from(E_FAIL));
        }
        self.registered.set(true);
        let mut version_data = data;
        version_data.Anonymous.uVersion = NOTIFYICON_VERSION_4;
        if !unsafe { Shell_NotifyIconW(NIM_SETVERSION, &version_data) }.as_bool() {
            tracing::warn!("设置通知区图标回调版本失败，将继续兼容旧版事件");
        }
        Ok(())
    }

    fn restore(&self) {
        self.remove();
        self.retries.set(0);
        if let Err(error) = self.install() {
            tracing::warn!(%error, "通知区尚未就绪，将在后台重试添加图标");
            if unsafe { SetTimer(Some(self.hwnd), RETRY_TIMER, RETRY_INTERVAL_MS, None) } == 0 {
                tracing::warn!("无法启动通知区重试计时器");
            }
        }
    }

    fn retry(&self) {
        if self.registered.get() || self.retries.get() >= MAX_RETRIES {
            return;
        }
        self.retries.set(self.retries.get() + 1);
        if self.install().is_ok() {
            let _ = unsafe { KillTimer(Some(self.hwnd), RETRY_TIMER) };
            tracing::info!("通知区图标已恢复");
        } else if self.retries.get() >= MAX_RETRIES {
            let _ = unsafe { KillTimer(Some(self.hwnd), RETRY_TIMER) };
            tracing::warn!("通知区暂不可用，等待资源管理器恢复后重新添加图标");
        }
    }

    fn remove(&self) {
        let _ = unsafe { KillTimer(Some(self.hwnd), RETRY_TIMER) };
        self.retries.set(MAX_RETRIES);
        if self.registered.replace(false) {
            let _ = unsafe { Shell_NotifyIconW(NIM_DELETE, &icon_identity(self.hwnd)) };
        }
    }

    fn menu(&self) {
        let state = self.state.get();
        let Some(menu) = (unsafe { CreatePopupMenu() }).ok() else {
            return;
        };
        let checked = |on| if on { MF_CHECKED } else { MF_STRING };
        let disabled = |on| if on { MF_STRING } else { MF_GRAYED };
        let add = |id, text, flags| {
            let _ = unsafe { AppendMenuW(menu, MF_STRING | flags, id, &HSTRING::from(text)) };
        };
        add(ID_VOICE, "语音输入（按住 Alt）", checked(state.voice_input));
        add(
            ID_PRACTICE,
            "启用跟读练习 / Reading practice",
            checked(state.english_practice) | disabled(state.voice_input),
        );
        add(
            ID_DIRECTION,
            "学习英语 / Learn English（关闭学中文）",
            checked(state.learn_english),
        );
        add(
            ID_DEMO,
            "跟读示范朗读",
            checked(state.practice_demo) | disabled(state.voice_input && state.english_practice),
        );
        add(ID_CLOUD, "联网翻译与联想", checked(state.cloud));
        add(ID_STATUS, "悬浮状态条", checked(state.status_bar));
        add(ID_PUNCTUATION, "全角标点", checked(state.full_width));
        let _ = unsafe { AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null()) };
        add(ID_SETTINGS, "进入设置…", MENU_ITEM_FLAGS(0));
        let mut point = POINT::default();
        let _ = unsafe { GetCursorPos(&mut point) };
        let selected = unsafe {
            let _ = SetForegroundWindow(self.hwnd);
            TrackPopupMenuEx(
                menu,
                (TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN).0,
                point.x,
                point.y,
                self.hwnd,
                None,
            )
        };
        unsafe {
            let _ = PostMessageW(Some(self.hwnd), WM_NULL, WPARAM(0), LPARAM(0));
        }
        unsafe {
            let _ = DestroyMenu(menu);
        }
        if selected.as_bool() {
            self.select(selected.0 as usize);
        }
    }

    fn select(&self, id: usize) {
        let action = match id {
            ID_VOICE => TrayAction::ToggleVoiceInput,
            ID_PRACTICE => TrayAction::ToggleEnglishPractice,
            ID_DIRECTION => TrayAction::ToggleLearningDirection,
            ID_DEMO => TrayAction::TogglePracticeDemo,
            ID_CLOUD => TrayAction::ToggleCloud,
            ID_STATUS => TrayAction::ToggleStatusBar,
            ID_PUNCTUATION => TrayAction::TogglePunctuation,
            ID_SETTINGS => TrayAction::OpenSettings,
            _ => return,
        };
        (self.events)(StatusEvent::Tray(action));
    }
}

impl Drop for TrayIcon {
    fn drop(&mut self) {
        self.remove();
        TRAYS.with(|trays| {
            trays.borrow_mut().remove(&(self.hwnd.0 as isize));
        });
        let _ = unsafe { DestroyWindow(self.hwnd) };
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let restored_message = taskbar_created();
    if restored_message != 0 && msg == restored_message {
        if let Some(tray) = tray_for(hwnd) {
            tray.restore();
        }
        return LRESULT(0);
    }
    if msg == WM_TIMER && wparam.0 == RETRY_TIMER {
        if let Some(tray) = tray_for(hwnd) {
            tray.retry();
        }
        return LRESULT(0);
    }
    // 注销时 detached UI 线程可能来不及执行 Drop，直接在窗口消息里通知 Shell 删除。
    if msg == WM_DESTROY || (msg == WM_ENDSESSION && wparam.0 != 0) {
        if let Some(tray) = tray_for(hwnd) {
            tray.remove();
        }
    }
    if msg == TRAY_MESSAGE || msg == WM_CONTEXTMENU {
        if let Some(tray) = tray_for(hwnd) {
            let event = if msg == WM_CONTEXTMENU {
                WM_CONTEXTMENU
            } else {
                (lparam.0 as u32) & 0xffff
            };
            tracing::debug!(message = msg, event, "收到通知区图标事件");
            match event {
                WM_RBUTTONUP | WM_CONTEXTMENU => tray.menu(),
                WM_LBUTTONDBLCLK => tray.select(ID_SETTINGS),
                WM_LBUTTONUP => {}
                _ => {}
            }
        }
        return LRESULT(0);
    }
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

fn icon_identity(hwnd: HWND) -> NOTIFYICONDATAW {
    NOTIFYICONDATAW {
        cbSize: core::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: ICON_ID,
        uFlags: NIF_GUID,
        guidItem: ICON_GUID,
        ..Default::default()
    }
}

fn tray_for(hwnd: HWND) -> Option<Rc<TrayIcon>> {
    TRAYS.with(|trays| {
        trays
            .borrow()
            .get(&(hwnd.0 as isize))
            .and_then(Weak::upgrade)
    })
}

fn taskbar_created() -> u32 {
    TASKBAR_CREATED.with(|message| {
        if message.get() == 0 {
            message.set(unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) });
        }
        message.get()
    })
}

#[cfg(test)]
mod tests {
    use super::{ICON_GUID, icon_identity};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Shell::NIF_GUID;

    #[test]
    fn tray_identity_survives_callback_window_recreation() {
        let first = icon_identity(HWND(17usize as *mut _));
        let restarted = icon_identity(HWND(23usize as *mut _));
        assert_ne!(first.hWnd, restarted.hWnd);
        assert_eq!(first.guidItem, restarted.guidItem);
        assert_eq!(first.guidItem, ICON_GUID);
        assert_ne!(first.uFlags & NIF_GUID, Default::default());
        assert_ne!(restarted.uFlags & NIF_GUID, Default::default());
    }
}
