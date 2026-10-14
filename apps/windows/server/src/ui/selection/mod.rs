//! 划词翻译：在任意软件里用鼠标选中文字后，鼠标旁浮出「译」小图标；点它借用剪贴板读出选区（读完放回原内容），
//! 交给本机离线模型翻译（不联网），译文显示在不抢焦点的小窗里，可一键复制。
//!
//! 全部窗口在 UI 线程上；钩子线程（[`hook`]）只投坐标消息。窗口过程只在队列消息（自定义消息、点击、计时器、
//! 重绘）里借用状态，调 Win32 引发的同步消息一律走默认处理，避免重入借用。

mod clipboard;
mod hook;
mod paint;

use std::cell::RefCell;
use std::time::{Duration, Instant};

use qingjian_core::Language;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::InvalidateRect;
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GA_ROOT, GetAncestor, GetClassNameW,
    GetForegroundWindow, GetWindowThreadProcessId, HWND_TOPMOST, IDC_HAND, KillTimer, LoadCursorW,
    MA_NOACTIVATE, SW_HIDE, SWP_NOACTIVATE, SWP_SHOWWINDOW, SetTimer, SetWindowPos, ShowWindow,
    WM_APP, WM_ERASEBKGND, WM_LBUTTONUP, WM_MOUSEACTIVATE, WM_PAINT, WM_TIMER, WNDCLASSEXW,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};
use windows::core::{PCWSTR, Result, w};

use self::clipboard::Snapshot;
use self::hook::SelectionHook;
use self::paint::{Fonts, PopupContent, PopupLayout};
use super::monitor;
use super::window_class::WindowClass;
use crate::voice::translation::Translator;

/// 钩子：像是刚选了文字，参数是松开鼠标的屏幕坐标。
const MSG_CANDIDATE: u32 = WM_APP + 0x61;

/// 钩子：任意鼠标键按下（点到别处就收起图标和小窗）。
const MSG_PRESS: u32 = WM_APP + 0x62;

const ICON_TIMER: usize = 1;
const CAPTURE_TIMER: usize = 2;
const POLL_TIMER: usize = 3;

/// 图标出现后多久没点就自动消失。
const ICON_LIFETIME_MS: u32 = 4_000;

/// 等前台软件把选区写进剪贴板的最长时间。
const CAPTURE_LIMIT: Duration = Duration::from_millis(900);

/// 剪贴板序号变化后再等这么久才读，应用可能分几步写入。
const CAPTURE_SETTLE: Duration = Duration::from_millis(60);

/// 等译文的最长时间：本机模型首次加载要几秒，长段落翻译也在一分钟内。
const TRANSLATE_LIMIT: Duration = Duration::from_secs(90);

/// 单次翻译的原文上限，与离线翻译器一致。
const MAX_CHARS: usize = 4000;

const CLASS_NAME: PCWSTR = w!("VakyuSelectionTranslate");
static CLASS: WindowClass = WindowClass::new();

/// 不弹图标的前台窗口：桌面、任务栏、资源管理器里拖选的是文件；终端里 Ctrl+C 可能是中断命令。
const IGNORED_CLASSES: [&str; 13] = [
    "Progman",
    "WorkerW",
    "Shell_TrayWnd",
    "Shell_SecondaryTrayWnd",
    "CabinetWClass",
    "ExploreWClass",
    "Windows.UI.Core.CoreWindow",
    "XamlExplorerHostIslandWindow",
    "ConsoleWindowClass",
    "CASCADIA_HOSTING_WINDOW_CLASS",
    "mintty",
    "PuTTY",
    "VirtualConsoleClass",
];

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

/// UI 线程持有的入口；丢弃时销毁窗口、卸下钩子。
pub(super) struct SelectionTranslate {
    icon: HWND,

    popup: HWND,
}

struct State {
    icon: HWND,

    popup: HWND,

    hook: Option<SelectionHook>,

    enabled: bool,

    translator: Option<Translator>,

    /// 本轮译文开始等待的时间；本机模型第一次加载较慢，超时才提示失败。
    waiting_since: Option<Instant>,

    fonts: Fonts,

    icon_rect: Option<RECT>,

    popup_rect: Option<RECT>,

    capture: Option<Capture>,

    request: u64,

    content: PopupContent,

    layout: PopupLayout,

    anchor: POINT,
}

/// 正在等前台软件响应 Ctrl+C。
struct Capture {
    snapshot: Snapshot,

    before: u32,

    started: Instant,

    /// 最近一次看到的序号与看到的时间。
    changed: Option<(u32, Instant)>,
}

impl SelectionTranslate {
    pub(super) fn new() -> Result<Self> {
        CLASS.ensure(|| WNDCLASSEXW {
            lpfnWndProc: Some(wndproc),
            hInstance: super::module_handle(),
            hCursor: unsafe { LoadCursorW(None, IDC_HAND) }.unwrap_or_default(),
            lpszClassName: CLASS_NAME,
            ..Default::default()
        })?;
        let icon = create_window(w!("Vakyu 划词翻译"))?;
        let popup = match create_window(w!("Vakyu 译文")) {
            Ok(popup) => popup,
            Err(error) => {
                let _ = unsafe { DestroyWindow(icon) };
                return Err(error);
            }
        };
        STATE.with(|state| {
            *state.borrow_mut() = Some(State {
                icon,
                popup,
                hook: None,
                enabled: false,
                translator: None,
                waiting_since: None,
                fonts: Fonts::new(96),
                icon_rect: None,
                popup_rect: None,
                capture: None,
                request: 0,
                content: PopupContent::default(),
                layout: PopupLayout::default(),
                anchor: POINT::default(),
            });
        });
        Ok(Self { icon, popup })
    }

    /// 开启时（离线模型齐全才装钩子）出现「译」图标；关闭时收起。状态没变就什么都不做。
    pub(super) fn configure(&self, enabled: bool) {
        with_state(|state| {
            if state.enabled == enabled && (!enabled || state.translator.is_some()) {
                return;
            }
            state.enabled = enabled;
            state.translator = None;
            state.hook = None;
            state.hide_icon();
            state.hide_popup();
            if !enabled {
                tracing::info!("划词翻译：已关闭");
                return;
            }
            match Translator::reading() {
                Ok(translator) => {
                    state.translator = Some(translator);
                    state.hook = SelectionHook::start(state.icon);
                    tracing::info!(hooked = state.hook.is_some(), "划词翻译：已开启");
                }
                Err(_) => tracing::info!("划词翻译：离线翻译模型不完整，暂不显示「译」图标"),
            }
        });
    }
}

impl Drop for SelectionTranslate {
    fn drop(&mut self) {
        STATE.with(|state| state.borrow_mut().take());
        unsafe {
            let _ = DestroyWindow(self.icon);
            let _ = DestroyWindow(self.popup);
        }
    }
}

fn create_window(title: PCWSTR) -> Result<HWND> {
    unsafe {
        CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            CLASS_NAME,
            title,
            WS_POPUP,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(super::module_handle()),
            None,
        )
    }
}

fn with_state(action: impl FnOnce(&mut State)) {
    STATE.with(|state| {
        if let Ok(mut state) = state.try_borrow_mut()
            && let Some(state) = state.as_mut()
        {
            action(state);
        }
    });
}

impl State {
    fn enabled(&self) -> bool {
        self.translator.is_some()
    }

    fn on_candidate(&mut self, point: POINT) {
        if !self.enabled() || self.capture.is_some() || inside(self.popup_rect, point) {
            return;
        }
        if !foreground_accepts_selection() {
            return;
        }
        self.sync_dpi(point);
        let size = paint::icon_size(self.fonts.dpi);
        let offset = paint::scale(12, self.fonts.dpi);
        let (x, y) = clamp(point.x + offset, point.y + offset, size, size, point);
        self.anchor = point;
        paint::round_corners(self.icon, size, size, paint::scale(7, self.fonts.dpi));
        self.icon_rect = Some(RECT {
            left: x,
            top: y,
            right: x + size,
            bottom: y + size,
        });
        show_at(self.icon, x, y, size, size);
        unsafe { SetTimer(Some(self.icon), ICON_TIMER, ICON_LIFETIME_MS, None) };
    }

    fn on_press(&mut self, point: POINT) {
        if !inside(self.icon_rect, point) {
            self.hide_icon();
        }
        if self.capture.is_none() && !inside(self.popup_rect, point) {
            self.hide_popup();
        }
    }

    /// 点了「译」：备份剪贴板 → Ctrl+C → 等选区写入。
    fn on_icon_click(&mut self) {
        self.hide_icon();
        if self.capture.is_some() || !self.enabled() {
            return;
        }
        self.request = self.request.wrapping_add(1);
        self.content = PopupContent {
            title: "Vakyu 翻译中…".to_owned(),
            text: "正在读取选中的文字…".to_owned(),
            ..PopupContent::default()
        };
        self.show_popup();
        let Some(snapshot) = clipboard::snapshot(self.icon) else {
            self.fail("剪贴板正被占用或内容太大，没有读取选中的文字。请稍后再试。");
            return;
        };
        let before = clipboard::sequence();
        if !clipboard::send_copy() {
            self.fail("无法向当前软件发送复制指令（可能是以管理员身份运行的软件）。");
            return;
        }
        self.capture = Some(Capture {
            snapshot,
            before,
            started: Instant::now(),
            changed: None,
        });
        unsafe { SetTimer(Some(self.icon), CAPTURE_TIMER, 30, None) };
    }

    fn on_capture_tick(&mut self) {
        let Some(capture) = &mut self.capture else {
            let _ = unsafe { KillTimer(Some(self.icon), CAPTURE_TIMER) };
            return;
        };
        let now = Instant::now();
        let current = clipboard::sequence();
        if current != capture.before {
            match capture.changed {
                Some((seen, at)) if seen == current => {
                    if now.duration_since(at) < CAPTURE_SETTLE {
                        return;
                    }
                }
                _ => {
                    capture.changed = Some((current, now));
                    return;
                }
            }
            let text = clipboard::read_text(self.icon);
            let capture = self.capture.take().expect("已检查读取进行中");
            let _ = unsafe { KillTimer(Some(self.icon), CAPTURE_TIMER) };
            clipboard::restore(self.icon, &capture.snapshot);
            match text.map(|text| text.trim().to_owned()) {
                Some(text) if !text.is_empty() => self.translate(text),
                _ => self.fail("选中的不是文字，或这个软件不允许复制。"),
            }
            return;
        }
        if now.duration_since(capture.started) >= CAPTURE_LIMIT {
            self.capture = None;
            let _ = unsafe { KillTimer(Some(self.icon), CAPTURE_TIMER) };
            self.fail("没有读到选中的文字：选区可能已取消，或这个软件不允许复制。");
        }
    }

    fn translate(&mut self, text: String) {
        if text.chars().count() > MAX_CHARS {
            self.content.source = text;
            self.fail("选中的文字超过 4000 字，请分段翻译。");
            return;
        }
        let target = qingjian_predict::reading_target(&text);
        self.content = PopupContent {
            title: title_for(target).to_owned(),
            source: text.clone(),
            text: "正在翻译…".to_owned(),
            ..PopupContent::default()
        };
        let request = self.request;
        let Some(translator) = self.translator.as_mut() else {
            return;
        };
        match translator.request_reading(request, &text) {
            Ok(_) => {
                self.waiting_since = Some(Instant::now());
                self.show_popup();
                unsafe { SetTimer(Some(self.popup), POLL_TIMER, 100, None) };
            }
            Err(message) => self.fail(&message),
        }
    }

    fn on_poll_tick(&mut self) {
        let Some(result) = self.translator.as_mut().and_then(Translator::try_recv) else {
            if self
                .waiting_since
                .is_some_and(|since| since.elapsed() >= TRANSLATE_LIMIT)
            {
                self.waiting_since = None;
                if let Some(translator) = &mut self.translator {
                    translator.cancel();
                }
                self.fail("翻译等待超时，请重试。");
            }
            return;
        };
        if result.request_id != self.request {
            return;
        }
        let _ = unsafe { KillTimer(Some(self.popup), POLL_TIMER) };
        self.waiting_since = None;
        match result.result {
            Ok(translation) => {
                self.content.text = translation;
                self.content.ready = true;
                self.show_popup();
            }
            Err(message) => self.fail(&message),
        }
    }

    fn on_popup_click(&mut self, x: i32, y: i32) {
        let point = POINT { x, y };
        if contains(self.layout.close, point) {
            self.hide_popup();
        } else if contains(self.layout.copy, point) && self.content.ready && !self.content.failed {
            self.content.copied = clipboard::write_text(self.popup, &self.content.text);
            invalidate(self.popup);
        }
    }

    fn fail(&mut self, message: &str) {
        self.waiting_since = None;
        self.content.title = "Vakyu · 翻译失败".to_owned();
        self.content.text = message.to_owned();
        self.content.failed = true;
        self.content.ready = false;
        let _ = unsafe { KillTimer(Some(self.popup), POLL_TIMER) };
        self.show_popup();
    }

    fn show_popup(&mut self) {
        self.sync_dpi(self.anchor);
        self.layout = paint::layout_popup(self.popup, &self.fonts, &self.content);
        let (width, height) = (self.layout.width, self.layout.height);
        let gap = paint::scale(16, self.fonts.dpi);
        let (x, y) = clamp(
            self.anchor.x - gap,
            self.anchor.y + gap,
            width,
            height,
            self.anchor,
        );
        paint::round_corners(self.popup, width, height, paint::scale(12, self.fonts.dpi));
        self.popup_rect = Some(RECT {
            left: x,
            top: y,
            right: x + width,
            bottom: y + height,
        });
        show_at(self.popup, x, y, width, height);
        invalidate(self.popup);
    }

    fn hide_icon(&mut self) {
        if self.icon_rect.take().is_some() {
            let _ = unsafe { KillTimer(Some(self.icon), ICON_TIMER) };
            let _ = unsafe { ShowWindow(self.icon, SW_HIDE) };
        }
    }

    fn hide_popup(&mut self) {
        self.waiting_since = None;
        if self.popup_rect.take().is_some() {
            let _ = unsafe { KillTimer(Some(self.popup), POLL_TIMER) };
            let _ = unsafe { ShowWindow(self.popup, SW_HIDE) };
        }
        // 关掉小窗就不再等这一轮译文。
        self.request = self.request.wrapping_add(1);
        if let Some(translator) = &mut self.translator {
            translator.cancel();
        }
    }

    fn sync_dpi(&mut self, point: POINT) {
        let dpi = monitor::dpi_near(point).unwrap_or(96).max(96);
        if dpi != self.fonts.dpi {
            self.fonts = Fonts::new(dpi);
        }
    }
}

fn title_for(target: Language) -> &'static str {
    if target == Language::English {
        "Vakyu · 译成英文"
    } else {
        "Vakyu · 译成中文"
    }
}

/// 前台是本进程（跟读浮窗、设置）或桌面、任务栏、资源管理器时不弹图标。
fn foreground_accepts_selection() -> bool {
    let foreground = unsafe { GetForegroundWindow() };
    if foreground.is_invalid() {
        return false;
    }
    let root = unsafe { GetAncestor(foreground, GA_ROOT) };
    let root = if root.is_invalid() { foreground } else { root };
    let mut process = 0u32;
    unsafe { GetWindowThreadProcessId(root, Some(&mut process)) };
    if process == unsafe { GetCurrentProcessId() } {
        return false;
    }
    let mut name = [0u16; 128];
    let length = unsafe { GetClassNameW(root, &mut name) }.max(0) as usize;
    let class = String::from_utf16_lossy(&name[..length.min(name.len())]);
    !IGNORED_CLASSES.contains(&class.as_str())
}

/// 窗口左上角夹进锚点所在显示器的工作区；放不下就翻到锚点上方。
fn clamp(x: i32, y: i32, width: i32, height: i32, anchor: POINT) -> (i32, i32) {
    let work = monitor::work_area_near(anchor);
    let x = x.clamp(work.left, (work.right - width).max(work.left));
    let y = if y + height > work.bottom {
        (anchor.y - height - (y - anchor.y)).max(work.top)
    } else {
        y.max(work.top)
    };
    (x, y)
}

fn inside(rect: Option<RECT>, point: POINT) -> bool {
    rect.is_some_and(|rect| contains(rect, point))
}

fn contains(rect: RECT, point: POINT) -> bool {
    point.x >= rect.left && point.x < rect.right && point.y >= rect.top && point.y < rect.bottom
}

fn show_at(hwnd: HWND, x: i32, y: i32, width: i32, height: i32) {
    let _ = unsafe {
        SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            x,
            y,
            width,
            height,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        )
    };
}

fn invalidate(hwnd: HWND) {
    let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
}

fn point_from(wparam: WPARAM, lparam: LPARAM) -> POINT {
    POINT {
        x: wparam.0 as isize as i32,
        y: lparam.0 as i32,
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_ERASEBKGND => LRESULT(1),
        MSG_CANDIDATE => {
            with_state(|state| state.on_candidate(point_from(wparam, lparam)));
            LRESULT(0)
        }
        MSG_PRESS => {
            with_state(|state| state.on_press(point_from(wparam, lparam)));
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            let (x, y) = (
                (lparam.0 & 0xFFFF) as i16 as i32,
                ((lparam.0 >> 16) & 0xFFFF) as i16 as i32,
            );
            with_state(|state| {
                if hwnd == state.icon {
                    state.on_icon_click();
                } else if hwnd == state.popup {
                    state.on_popup_click(x, y);
                }
            });
            LRESULT(0)
        }
        WM_TIMER => {
            with_state(|state| match wparam.0 {
                ICON_TIMER => state.hide_icon(),
                CAPTURE_TIMER => state.on_capture_tick(),
                POLL_TIMER => state.on_poll_tick(),
                _ => {}
            });
            LRESULT(0)
        }
        WM_PAINT => {
            let mut painted = false;
            with_state(|state| {
                if hwnd == state.icon {
                    paint::paint_icon(hwnd, &state.fonts);
                    painted = true;
                } else if hwnd == state.popup {
                    paint::paint_popup(hwnd, &state.fonts, &state.content, &state.layout);
                    painted = true;
                }
            });
            if painted {
                LRESULT(0)
            } else {
                unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

#[cfg(test)]
mod tests {
    use super::{contains, title_for};
    use qingjian_core::Language;
    use windows::Win32::Foundation::{POINT, RECT};

    #[test]
    fn hit_testing_and_titles() {
        let rect = RECT {
            left: 10,
            top: 10,
            right: 20,
            bottom: 20,
        };
        assert!(contains(rect, POINT { x: 10, y: 19 }));
        assert!(!contains(rect, POINT { x: 20, y: 15 }));
        assert_eq!(title_for(Language::English), "Vakyu · 译成英文");
        assert_eq!(title_for(Language::Chinese), "Vakyu · 译成中文");
    }
}
