//! 语音与英语跟读浮窗：固定在当前屏幕底部中央、任务栏上方，不激活当前应用。

mod frame;
mod live;
mod meter;
mod practice;

use std::cell::{Cell, RefCell};
use std::time::{Duration, Instant};

use qingjian_platform::ThemeMode;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateRoundRectRgn, CreateSolidBrush, DeleteObject, FillRgn, GetDC, HDC, ReleaseDC, SetBkMode,
    TRANSPARENT,
};
use windows::Win32::UI::HiDpi::GetDpiForSystem;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LMENU, VK_MENU, VK_RMENU};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GWL_EXSTYLE, GWLP_USERDATA, GetCursorPos,
    GetWindowLongPtrW, HTCLIENT, HTTRANSPARENT, KillTimer, MA_NOACTIVATE, SW_HIDE, SW_SHOWNA,
    SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SetTimer,
    SetWindowLongPtrW, SetWindowPos, ShowWindow, WM_LBUTTONUP, WM_MOUSEACTIVATE, WM_NCHITTEST,
    WM_TIMER, WNDCLASSEXW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::{PCWSTR, Result, w};

use self::frame::VoiceFrame;
use self::meter::VoiceMeter;
use super::StatusEvents;
use super::candidates::resolve_dark;
use super::candidates::theme::Theme;
use super::candidates::view;
use super::layered::{self, Layered};
use super::monitor;
use super::window_class::WindowClass;
use crate::dispatch::{PracticeView, RecordingMode};

struct VoiceEvents {
    callback: StatusEvents,

    practice_view: RefCell<Option<PracticeView>>,

    practice_content: Cell<(i32, i32)>,

    practice_word_hits: RefCell<Vec<(RECT, usize)>>,

    practice_dpi: Cell<u32>,
}

const CLASS_NAME: PCWSTR = w!("MiracleVoiceWindow");
static CLASS: WindowClass = WindowClass::new();
const MAX_LINES: usize = 3;
const HELD_TIMER: usize = 1;
const HELD_LEASE: Duration = Duration::from_millis(250);

thread_local! {
    /// None 保留旧输入法入口；全局入口一旦接管，只信后台持续续约的生命周期。
    static GLOBAL_HELD: Cell<Option<(bool, Instant)>> = const { Cell::new(None) };

    /// 跟读由显式取消或交付结果收尾，不能因同步 UIA/TSF 等待超过录音租约而消失。
    static PRACTICE_VISIBLE: Cell<bool> = const { Cell::new(false) };
}

/// 只接收 UI 线程命令的语音浮窗；创建时隐藏，松开按键后立即隐藏。
pub(super) struct VoiceWindow {
    hwnd: HWND,

    events: Box<VoiceEvents>,

    theme: RefCell<Theme>,

    dpi: Cell<u32>,

    dark: Cell<bool>,

    frame: RefCell<Option<VoiceFrame>>,

    recording: Cell<bool>,

    meter: RefCell<VoiceMeter>,

    level_dirty: Cell<bool>,

    last_level_render: Cell<Option<Instant>>,

    /// 跟读识别更新可能高于显示器刷新率；保留最新帧，最多按 30fps 做一次整窗合成。
    last_practice_render: Cell<Option<Instant>>,

    /// 起始词变色的时间；跟读状态更新时重置，避免一次性整窗重绘带来的闪变。
    word_transition_started: Cell<Option<Instant>>,

    /// 上一次提交给练习窗口的已读词数；用于只在词进度变化时重置过渡动画。
    last_practice_matched: Cell<Option<usize>>,
}

impl VoiceWindow {
    pub(super) fn new(events: StatusEvents) -> Result<Self> {
        CLASS.ensure(|| WNDCLASSEXW {
            lpfnWndProc: Some(wndproc),
            hInstance: super::module_handle(),
            lpszClassName: CLASS_NAME,
            ..Default::default()
        })?;
        let dpi = unsafe { GetDpiForSystem() }.max(96);
        let dark = resolve_dark(ThemeMode::default());
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_LAYERED
                    | WS_EX_TOOLWINDOW
                    | WS_EX_TOPMOST
                    | WS_EX_NOACTIVATE
                    | WS_EX_TRANSPARENT,
                CLASS_NAME,
                w!("Vakyu 语音输入"),
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
        let events = Box::new(VoiceEvents {
            callback: events,
            practice_view: RefCell::new(None),
            practice_content: Cell::new((0, 0)),
            practice_word_hits: RefCell::new(Vec::new()),
            practice_dpi: Cell::new(dpi),
        });
        unsafe {
            SetWindowLongPtrW(
                hwnd,
                GWLP_USERDATA,
                (&*events as *const VoiceEvents) as isize,
            );
        }
        Ok(Self {
            hwnd,
            events,
            theme: RefCell::new(Theme::new(dpi, dark)),
            dpi: Cell::new(dpi),
            dark: Cell::new(dark),
            frame: RefCell::new(None),
            recording: Cell::new(false),
            meter: RefCell::new(VoiceMeter::default()),
            level_dirty: Cell::new(false),
            last_level_render: Cell::new(None),
            last_practice_render: Cell::new(None),
            word_transition_started: Cell::new(None),
            last_practice_matched: Cell::new(None),
        })
    }

    pub(super) fn show(
        &self,
        text: String,
        anchor: RECT,
        status: &str,
        recording: bool,
        mode: RecordingMode,
    ) {
        self.show_bubble(text, anchor, status, recording, mode, mode.release_hint());
    }

    pub(super) fn show_error(&self, text: String, anchor: RECT, mode: RecordingMode) {
        self.show_bubble(
            text,
            anchor,
            "语音暂不可用",
            false,
            mode,
            "松开结束，检查提示后重试",
        );
    }

    fn show_bubble(
        &self,
        text: String,
        anchor: RECT,
        status: &str,
        recording: bool,
        mode: RecordingMode,
        release_hint: &'static str,
    ) {
        PRACTICE_VISIBLE.set(false);
        self.set_transparent(true);
        self.events.practice_view.borrow_mut().take();
        if !voice_held() {
            self.hide();
            return;
        }
        self.cache_frame(
            VoiceFrame::Bubble {
                text: text.clone(),
                anchor,
                status: status.to_owned(),
                mode,
                release_hint,
            },
            recording,
        );
        self.sync_theme(anchor);
        if let Err(error) = self.render(&text, anchor, status, mode, release_hint) {
            tracing::warn!(%error, "绘制语音浮窗失败");
            self.hide();
            return;
        }
        let _ = unsafe { ShowWindow(self.hwnd, SW_SHOWNA) };
        // 控制线程失联也不能留下浮窗；此计时器只管展示，不能改变录音生命周期。
        unsafe { SetTimer(Some(self.hwnd), HELD_TIMER, 60, None) };
    }

    pub(super) fn set_held(&self, held: bool) {
        if held && !voice_held() {
            self.hide();
        }
        GLOBAL_HELD.set(Some((held, Instant::now())));
        if !held {
            self.hide();
        }
    }

    pub(super) fn hide(&self) {
        PRACTICE_VISIBLE.set(false);
        self.set_transparent(true);
        self.meter.borrow_mut().clear();
        self.recording.set(false);
        self.frame.borrow_mut().take();
        self.level_dirty.set(false);
        self.last_level_render.set(None);
        self.last_practice_render.set(None);
        self.word_transition_started.set(None);
        self.last_practice_matched.set(None);
        self.events.practice_view.borrow_mut().take();
        self.events.practice_word_hits.borrow_mut().clear();
        let _ = unsafe { KillTimer(Some(self.hwnd), HELD_TIMER) };
        let _ = unsafe { ShowWindow(self.hwnd, SW_HIDE) };
    }

    fn set_transparent(&self, transparent: bool) {
        let style = unsafe { GetWindowLongPtrW(self.hwnd, GWL_EXSTYLE) };
        let transparent_flag = WS_EX_TRANSPARENT.0 as isize;
        let style = if transparent {
            style | transparent_flag
        } else {
            style & !transparent_flag
        };
        unsafe {
            SetWindowLongPtrW(self.hwnd, GWL_EXSTYLE, style);
            let _ = SetWindowPos(
                self.hwnd,
                None,
                0,
                0,
                0,
                0,
                SWP_FRAMECHANGED | SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER,
            );
        }
    }

    pub(super) fn word_transition_progress(&self) -> f32 {
        let start = self.word_transition_started.get().unwrap_or_else(|| {
            let now = Instant::now();
            self.word_transition_started.set(Some(now));
            now
        });
        (start.elapsed().as_secs_f32() / 0.16).min(1.0)
    }

    pub(super) fn reset_word_transition(&self) {
        self.word_transition_started.set(Some(Instant::now()));
    }

    fn sync_theme(&self, anchor: RECT) {
        let dpi = monitor::dpi_near(anchor_point(anchor))
            .unwrap_or(self.dpi.get())
            .max(96);
        let dark = resolve_dark(ThemeMode::default());
        if dpi != self.dpi.get() || dark != self.dark.get() {
            *self.theme.borrow_mut() = Theme::new(dpi, dark);
            self.dpi.set(dpi);
            self.dark.set(dark);
        }
    }

    fn render(
        &self,
        text: &str,
        anchor: RECT,
        status: &str,
        mode: RecordingMode,
        release_hint: &str,
    ) -> Result<()> {
        let dpi = self.dpi.get();
        let scale = |px: i32| (px * dpi as i32 / 96).max(1);
        let theme = self.theme.borrow();
        let margin = layered::shadow_margin(dpi);
        let work = monitor::work_area_near(anchor_point(anchor));
        let width = scale(380).min((work.right - work.left - 2 * margin).max(scale(220)));
        let padding = scale(18);
        let hdc = unsafe { GetDC(Some(self.hwnd)) };
        let display_text = if text.trim().is_empty() {
            mode.placeholder()
        } else {
            text
        };
        let lines = transcript_lines(hdc, &theme, display_text, width - padding * 2);
        let status = fit_status(hdc, &theme, status, width - padding * 2);
        let text_height = view::measure(hdc, theme.text_font, "中文")
            .cy
            .max(scale(18));
        let hint_height = view::measure(hdc, theme.annotation_font, release_hint).cy;
        unsafe { ReleaseDC(Some(self.hwnd), hdc) };
        let line_height = text_height + scale(6);
        let transcript_top = scale(74);
        let hint_top = transcript_top + line_height * lines.len() as i32 + scale(8);
        let content = (width, hint_top + hint_height + scale(16));
        let position = place(work, content, margin, scale(16));
        layered::composite(
            self.hwnd,
            &Layered {
                content,
                margin,
                win_pos: (position.0 - margin, position.1 - margin),
                win_size: (content.0 + margin * 2, content.1 + margin * 2),
                background: theme.background,
                background_end: theme.background_end,
                corner_radius: scale(16),
                paint: &|hdc, _| {
                    unsafe { SetBkMode(hdc, TRANSPARENT) };
                    paint_header(hdc, &theme, dpi, &status, mode, &self.meter.borrow());
                    for (index, line) in lines.iter().enumerate() {
                        view::draw_text(
                            hdc,
                            theme.text_font,
                            if text.trim().is_empty() {
                                theme.gloss_color
                            } else {
                                theme.text_color
                            },
                            padding,
                            transcript_top + index as i32 * line_height,
                            line,
                        );
                    }
                    view::draw_text(
                        hdc,
                        theme.annotation_font,
                        theme.gloss_color,
                        padding,
                        hint_top,
                        release_hint,
                    );
                },
            },
        )
    }
}

impl Drop for VoiceWindow {
    fn drop(&mut self) {
        PRACTICE_VISIBLE.set(false);
        let _ = unsafe { DestroyWindow(self.hwnd) };
    }
}

/// 只保留最新三行，长时间说话仍能看到正在识别的句尾；不更改实际转写结果。
fn transcript_lines(hdc: HDC, theme: &Theme, text: &str, width: i32) -> Vec<String> {
    if text.trim().is_empty() {
        return vec!["请说话，文字会实时显示在这里…".to_owned()];
    }
    // 限制每帧排版工作量。256 个字符远多于浮窗能显示的三行。
    let tail: String = text
        .chars()
        .rev()
        .take(256)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let mut lines = Vec::new();
    let mut line = String::new();
    for ch in tail.chars() {
        if ch == '\r' {
            continue;
        }
        if ch == '\n' {
            lines.push(std::mem::take(&mut line));
            continue;
        }
        line.push(ch);
        if view::measure(hdc, theme.text_font, &line).cx > width && line.chars().count() > 1 {
            line.pop();
            lines.push(std::mem::take(&mut line));
            line.push(ch);
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    if lines.len() > MAX_LINES {
        lines.drain(..lines.len() - MAX_LINES);
    }
    lines
}

fn fit_status(hdc: HDC, theme: &Theme, status: &str, width: i32) -> String {
    let status = status.replace(['\r', '\n'], " ");
    if view::measure(hdc, theme.annotation_font, &status).cx <= width {
        return status;
    }
    let mut fitted: String = status.chars().take(128).collect();
    while !fitted.is_empty() {
        fitted.pop();
        let candidate = format!("{fitted}…");
        if view::measure(hdc, theme.annotation_font, &candidate).cx <= width {
            return candidate;
        }
    }
    "…".to_owned()
}

fn paint_header(
    hdc: HDC,
    theme: &Theme,
    dpi: u32,
    status: &str,
    mode: RecordingMode,
    meter: &VoiceMeter,
) {
    let scale = |px: i32| (px * dpi as i32 / 96).max(1);
    let accent = rgb(0xff, 0xff, 0xff);
    paint_meter(hdc, dpi, scale(18), scale(28), meter);
    view::draw_text(
        hdc,
        theme.text_font,
        accent,
        scale(52),
        scale(20),
        mode.title(),
    );
    view::draw_text(
        hdc,
        theme.annotation_font,
        theme.gloss_color,
        scale(18),
        scale(49),
        status,
    );
}

fn paint_meter(hdc: HDC, dpi: u32, left: i32, center: i32, meter: &VoiceMeter) {
    let scale = |px: i32| (px * dpi as i32 / 96).max(1);
    let accent = rgb(0xf1, 0xdf, 0xff);
    if meter.quiet() {
        fill_round(
            hdc,
            RECT {
                left,
                top: center - scale(1),
                right: left + scale(23),
                bottom: center + scale(1),
            },
            accent,
            scale(1),
        );
        return;
    }
    for (index, height) in meter.heights().into_iter().enumerate() {
        let x = left + index as i32 * scale(5);
        let top = center - scale(height) / 2;
        fill_round(
            hdc,
            RECT {
                left: x,
                top,
                right: x + scale(3),
                bottom: top + scale(height),
            },
            accent,
            scale(2),
        );
    }
}

fn fill_round(hdc: HDC, rect: RECT, color: COLORREF, radius: i32) {
    unsafe {
        let region = CreateRoundRectRgn(
            rect.left,
            rect.top,
            rect.right,
            rect.bottom,
            radius * 2,
            radius * 2,
        );
        let brush = CreateSolidBrush(color);
        let _ = FillRgn(hdc, region, brush);
        let _ = DeleteObject(brush.into());
        let _ = DeleteObject(region.into());
    }
}

const fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    COLORREF(r as u32 | ((g as u32) << 8) | ((b as u32) << 16))
}

fn place(work: RECT, content: (i32, i32), margin: i32, gap: i32) -> (i32, i32) {
    // 工作区排除任务栏；底边固定，转写增加行数时只向上长，阴影也留在工作区内。
    (
        work.left + (work.right - work.left - content.0) / 2,
        (work.bottom - content.1 - margin - gap).max(work.top + margin),
    )
}

fn empty_anchor(anchor: RECT) -> bool {
    anchor.left == 0 && anchor.top == 0 && anchor.right == 0 && anchor.bottom == 0
}

fn anchor_point(anchor: RECT) -> POINT {
    if empty_anchor(anchor) {
        let mut point = POINT::default();
        let _ = unsafe { GetCursorPos(&mut point) };
        point
    } else {
        POINT {
            x: anchor.left,
            y: anchor.top,
        }
    }
}

fn global_held_at(state: Option<(bool, Instant)>, now: Instant) -> Option<bool> {
    state.map(|(held, renewed)| held && now.saturating_duration_since(renewed) < HELD_LEASE)
}

fn window_held_at(
    state: Option<(bool, Instant)>,
    now: Instant,
    practice_visible: bool,
) -> Option<bool> {
    if practice_visible {
        Some(state.is_none_or(|(held, _)| held))
    } else {
        global_held_at(state, now)
    }
}

fn voice_held() -> bool {
    window_held_at(GLOBAL_HELD.get(), Instant::now(), PRACTICE_VISIBLE.get()).unwrap_or_else(|| {
        [VK_MENU, VK_LMENU, VK_RMENU]
            .into_iter()
            .any(|key| unsafe { GetAsyncKeyState(i32::from(key.0)) < 0 })
    })
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_NCHITTEST => {
            let state = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *const VoiceEvents;
            let practice = !state.is_null() && unsafe { &*state }.practice_view.borrow().is_some();
            LRESULT(if practice {
                HTCLIENT as isize
            } else {
                HTTRANSPARENT as isize
            })
        }
        WM_LBUTTONUP => {
            let state = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *const VoiceEvents;
            if !state.is_null() {
                let state = unsafe { &*state };
                let x = (lparam.0 as u16 as i16) as i32;
                let y = ((lparam.0 >> 16) as u16 as i16) as i32;
                let dpi = state.practice_dpi.get();
                let margin = layered::shadow_margin(dpi);
                if let Some(view) = state.practice_view.borrow().as_ref()
                    && let Some(action) = practice::action_at(
                        view,
                        x - margin,
                        y - margin,
                        state.practice_content.get(),
                        dpi,
                    )
                    .or_else(|| {
                        practice::word_at(
                            view,
                            x - margin,
                            y - margin,
                            &state.practice_word_hits.borrow(),
                        )
                    })
                {
                    (state.callback)(crate::dispatch::StatusEvent::Practice(action));
                }
            }
            LRESULT(0)
        }
        WM_TIMER if wparam.0 == HELD_TIMER => {
            if !voice_held() {
                let _ = unsafe { KillTimer(Some(hwnd), HELD_TIMER) };
                let _ = unsafe { ShowWindow(hwnd, SW_HIDE) };
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod recording_tests;
