//! 悬浮状态条：桌面上常驻、可拖动的五格浮窗 `[中 / 英][，。/ ,.][跟读][发英][⚙]`。缺省由 Miracle 渲染器画（[`super::painter`]），
//! `renderer = "system"` 时复用分层窗口合成器与候选窗口的 GDI 主题。
//!
//! 按下鼠标先 `DragDetect`：挪出拖动阈值就交给系统的移动循环（`WM_NCLBUTTONDOWN` + `HTCAPTION`），
//! 结束时 `WM_EXITSIZEMOVE` 报新位置；没挪就是点击，按 x 落进哪格。`WM_MOUSEACTIVATE` 回 `MA_NOACTIVATE` 不抢焦点。
//! 一格的规格在 [`cell`]，摆放与点击在 [`placement`]。

mod cell;
mod fullscreen;
mod logo;
mod placement;
#[cfg(test)]
mod tests;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use windows::Win32::Foundation::{
    COLORREF, E_INVALIDARG, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM,
};
use windows::Win32::Graphics::Gdi::{GetDC, HDC, ReleaseDC, SetBkMode, TRANSPARENT};
use windows::Win32::UI::HiDpi::{GetDpiForSystem, GetDpiForWindow};
use windows::Win32::UI::Input::KeyboardAndMouse::{DragDetect, ReleaseCapture};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetCursorPos, HTCAPTION, HTCLIENT, IDC_HAND,
    LoadCursorW, MA_NOACTIVATE, SendMessageW, WM_EXITSIZEMOVE, WM_LBUTTONDOWN, WM_MOUSEACTIVATE,
    WM_NCHITTEST, WM_NCLBUTTONDOWN, WM_TIMER, WNDCLASSEXW, WS_EX_LAYERED, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};
use windows::core::{Error, PCWSTR, Result, w};

use qingjian_platform::ThemeMode;
use qingjian_render::StatusCell;

use self::cell::CellSpec;
use self::placement::{Placement, StatusAction};
use super::StatusEvents;
use super::candidates::resolve_dark;
use super::candidates::theme::Theme;
use super::candidates::view;
use super::layered::{self, Layered};
use super::monitor;
use super::painter::SharedPainter;
use super::window_class::WindowClass;
use crate::dispatch::StatusView;

const CLASS_NAME: PCWSTR = w!("MiracleStatusBar");
static CLASS: WindowClass = WindowClass::new();

/// 状态条与屏幕边缘的间隙（逻辑像素）。
const EDGE_GAP: i32 = 8;

thread_local! {
    /// 本线程活着的状态条：HWND → 摆放状态。窗口过程按 HWND 查，查不到（已析构）就忽略。
    static PLACEMENTS: RefCell<HashMap<isize, Rc<Placement>>> = RefCell::new(HashMap::new());
}

/// 悬浮状态条窗口。
pub(super) struct StatusBar {
    hwnd: HWND,

    /// 最近一次要显示的内容；还没显示过时为 `None`。
    data: RefCell<Option<StatusView>>,

    /// 按 DPI / 深浅造好的主题（复用候选窗口那套）。
    theme: RefCell<Rc<Theme>>,

    /// 上次用的 DPI，变了重建主题。
    dpi: Cell<u32>,

    /// 上次解析出的深浅，变了重建配色。
    dark: Cell<bool>,

    /// 摆放状态，与窗口过程共享。
    placement: Rc<Placement>,

    /// Miracle 渲染器；`None` 走 GDI。
    painter: SharedPainter,
}

/// 各格从左到右的动作；[`StatusBar::status_cells`] 与 [`StatusBar::cells`] 按同一顺序出格。
const ACTIONS: [StatusAction; 5] = [
    StatusAction::ToggleMode,
    StatusAction::TogglePunctuation,
    StatusAction::TogglePractice,
    StatusAction::ToggleSendTranslation,
    StatusAction::OpenSettings,
];

impl StatusBar {
    /// 建一个隐藏的状态条窗口。
    pub(super) fn new(events: StatusEvents, painter: SharedPainter) -> Result<Self> {
        CLASS.ensure(|| WNDCLASSEXW {
            lpfnWndProc: Some(wndproc),
            hInstance: super::module_handle(),
            hCursor: unsafe { LoadCursorW(None, IDC_HAND) }.unwrap_or_default(),
            lpszClassName: CLASS_NAME,
            ..Default::default()
        })?;
        let dpi = unsafe { GetDpiForSystem() }.max(96);
        let dark = resolve_dark(ThemeMode::default());
        // NOACTIVATE：显示时不抢应用焦点。
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE,
                CLASS_NAME,
                w!("Vakyu 状态条"),
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
        let placement = Rc::new(Placement::new(hwnd, layered::shadow_margin(dpi), events));
        PLACEMENTS.with(|map| map.borrow_mut().insert(hwnd.0 as isize, placement.clone()));
        Ok(Self {
            hwnd,
            data: RefCell::new(None),
            theme: RefCell::new(Rc::new(Theme::new(dpi, dark))),
            dpi: Cell::new(dpi),
            dark: Cell::new(dark),
            placement,
            painter,
        })
    }

    /// 显示 / 更新：按记住的位置（首次用 `view.anchor`，都没有就右下角）摆放并重绘。
    pub(super) fn update(&self, view: StatusView) {
        if self.placement.pos.get().is_none() {
            self.placement.pos.set(view.anchor);
        }
        *self.data.borrow_mut() = Some(view);
        self.sync_theme();
        self.render();
    }

    pub(super) fn hide(&self) {
        fullscreen::hide(self.hwnd, &self.placement.fullscreen_hidden);
    }

    /// DPI 或深浅变了就重建主题。DPI 优先取所在位置显示器的，理由同候选窗口（#146）。
    fn sync_theme(&self) {
        let monitor_dpi = self
            .placement
            .pos
            .get()
            .and_then(|(x, y)| monitor::dpi_near(POINT { x, y }));
        let dpi = match (monitor_dpi, unsafe { GetDpiForWindow(self.hwnd) }) {
            (Some(dpi), _) => dpi,
            (None, 0) => self.dpi.get(),
            (None, dpi) => dpi,
        };
        let mode = self
            .data
            .borrow()
            .as_ref()
            .map(|view| view.theme)
            .unwrap_or_default();
        let dark = resolve_dark(mode);
        if dpi != self.dpi.get() || dark != self.dark.get() {
            *self.theme.borrow_mut() = Rc::new(Theme::new(dpi, dark));
            self.dpi.set(dpi);
            self.dark.set(dark);
        }
    }

    /// 模式格的文字：中 / 英 / 注，开着双拼时跟方案名。
    fn mode_text(view: &StatusView) -> String {
        if view.english {
            "英".to_owned()
        } else if view.zhuyin {
            "注".to_owned()
        } else {
            match &view.scheme {
                Some(scheme) => format!("中 · {scheme}"),
                None => "中".to_owned(),
            }
        }
    }

    /// 「跟读 / 发英」两格的文字与是否点亮：两格互斥，跟读关着时都是灰的。
    fn practice_cells(view: &StatusView) -> [(&'static str, bool); 2] {
        let send_label = if view.learn_english {
            "发英"
        } else {
            "发中"
        };
        [
            ("跟读", view.practice && !view.send_translation),
            (send_label, view.practice && view.send_translation),
        ]
    }

    /// 渲染器要的五格：模式（品牌色）、标点 / 跟读 / 发英（生效时品牌色，否则灰）、齿轮。
    fn status_cells(view: &StatusView) -> Vec<StatusCell> {
        let [shadowing, sending] = Self::practice_cells(view);
        vec![
            StatusCell::text(Self::mode_text(view), true),
            StatusCell::text(if view.full_width { "，。" } else { ",." }, view.full_width),
            StatusCell::text(shadowing.0, shadowing.1),
            StatusCell::text(sending.0, sending.1),
            StatusCell::Gear,
        ]
    }

    /// GDI 画法的五格，顺序同 [`ACTIONS`]。
    fn cells(&self, theme: &Theme) -> Vec<CellSpec> {
        let data = self.data.borrow();
        let Some(view) = data.as_ref() else {
            return Vec::new();
        };
        let punctuation_active = view.full_width;
        let toggle = |(text, active): (&str, bool), action| CellSpec {
            text: text.to_owned(),
            font: theme.text_font,
            color: if active {
                theme.cloud_color
            } else {
                theme.gloss_color
            },
            action,
        };
        let [shadowing, sending] = Self::practice_cells(view);
        vec![
            CellSpec {
                text: Self::mode_text(view),
                font: theme.text_font,
                color: theme.cloud_color,
                action: StatusAction::ToggleMode,
            },
            CellSpec {
                text: if punctuation_active { "，。" } else { ",." }.to_owned(),
                font: theme.text_font,
                color: if punctuation_active {
                    theme.cloud_color
                } else {
                    theme.gloss_color
                },
                action: StatusAction::TogglePunctuation,
            },
            toggle(shadowing, StatusAction::TogglePractice),
            toggle(sending, StatusAction::ToggleSendTranslation),
            CellSpec {
                text: "\u{2699}".to_owned(),
                font: theme.symbol_font,
                color: theme.gloss_color,
                action: StatusAction::OpenSettings,
            },
        ]
    }

    /// 画好贴上并显示；顺带记下各格边界给点击用。渲染器画不成就走 GDI。
    fn render(&self) {
        let rendered = {
            let data = self.data.borrow();
            let mut painter = self.painter.borrow_mut();
            match (data.as_ref(), painter.as_mut()) {
                (Some(view), Some(painter)) => painter.render_status(
                    &Self::status_cells(view),
                    self.dark.get(),
                    self.dpi.get(),
                    logo::TOP_INSET,
                ),
                _ => None,
            }
        };
        let updated = match rendered {
            Some(rendered) => {
                let bitmap = &rendered.rendered;
                let content = (bitmap.content_width as i32, bitmap.content_height as i32);
                if content.0 <= 0 || content.1 <= 0 {
                    self.hide();
                    return;
                }
                let margin = bitmap.content_x as i32;
                self.placement.margin.set(margin);
                self.placement
                    .controls_y
                    .set((logo::inset(self.dpi.get()), content.1));
                *self.placement.cells.borrow_mut() = rendered
                    .cell_edges
                    .iter()
                    .zip(ACTIONS)
                    .map(|(edge, action)| (edge.round() as i32, action))
                    .collect();
                let anchor = self.anchor(content, margin);
                let win_pos = (anchor.0 - margin, anchor.1 - margin);
                logo::with_mark(self.dpi.get(), |mark| {
                    layered::present_with_overlay(
                        self.hwnd,
                        &bitmap.pixmap,
                        win_pos,
                        Some(layered::PixmapOverlay {
                            pixmap: mark,
                            x: margin + logo::left(self.dpi.get()),
                            y: margin - mark.height() as i32 / 2,
                        }),
                    )
                })
                .unwrap_or_else(|| layered::present(self.hwnd, &bitmap.pixmap, win_pos))
            }
            None => self.render_gdi(),
        };
        if updated.is_ok() {
            fullscreen::show(self.hwnd, &self.placement.fullscreen_hidden);
        } else {
            self.hide();
        }
    }

    /// 内容左上角：记住的位置，没有就右下角，再夹进工作区；顺带记下。
    fn anchor(&self, content: (i32, i32), margin: i32) -> (i32, i32) {
        let anchor = self
            .placement
            .pos
            .get()
            .unwrap_or_else(|| default_anchor(content, margin));
        let anchor = clamp_anchor(anchor, content, margin);
        self.placement.pos.set(Some(anchor));
        anchor
    }

    /// GDI 画法：量各格、算内容尺寸、摆位置、合成贴上。
    fn render_gdi(&self) -> Result<()> {
        let theme = self.theme.borrow().clone();
        let margin = layered::shadow_margin(self.dpi.get());
        self.placement.margin.set(margin);
        let cells = self.cells(&theme);
        let hdc = unsafe { GetDC(Some(self.hwnd)) };
        let sizes: Vec<SIZE> = cells
            .iter()
            .map(|cell| view::measure(hdc, cell.font, &cell.text))
            .collect();
        unsafe { ReleaseDC(Some(self.hwnd), hdc) };
        let line = sizes.iter().map(|size| size.cy).max().unwrap_or(0);
        // 每格：左右各一个 padding；格间一条细线。
        let widths: Vec<i32> = sizes
            .iter()
            .map(|size| size.cx + theme.padding * 2)
            .collect();
        let top_inset = logo::inset(self.dpi.get());
        let content = (widths.iter().sum::<i32>(), line + theme.padding + top_inset);
        if content.0 <= 0 || content.1 <= 0 || cells.is_empty() {
            return Err(Error::from(E_INVALIDARG));
        }
        let mut right = 0;
        let bounds: Vec<(i32, StatusAction)> = cells
            .iter()
            .zip(&widths)
            .map(|(cell, width)| {
                right += width;
                (right, cell.action)
            })
            .collect();
        *self.placement.cells.borrow_mut() = bounds;
        self.placement.controls_y.set((top_inset, content.1));
        let anchor = self.anchor(content, margin);

        let separator = theme.pos_color;
        let inset = theme.padding / 2;
        let paint = |hdc, mut client: RECT| {
            client.top = top_inset;
            unsafe { SetBkMode(hdc, TRANSPARENT) };
            paint_cells(hdc, client, &cells, &sizes, &widths, separator, inset);
        };
        let frame = Layered {
            content,
            margin,
            win_pos: (anchor.0 - margin, anchor.1 - margin),
            win_size: (content.0 + margin * 2, content.1 + margin * 2),
            background: theme.background,
            background_end: theme.background_end,
            corner_radius: theme.corner_radius,
            paint: &paint,
        };
        logo::with_mark(self.dpi.get(), |mark| {
            layered::composite_with_overlay(
                self.hwnd,
                &frame,
                Some(layered::PixmapOverlay {
                    pixmap: mark,
                    x: margin + logo::left(self.dpi.get()),
                    y: margin - mark.height() as i32 / 2,
                }),
            )
        })
        .unwrap_or_else(|| layered::composite(self.hwnd, &frame))
    }
}

/// 每格文字居中；格与格之间一条上下留 `inset` 的细线。
fn paint_cells(
    hdc: HDC,
    client: RECT,
    cells: &[CellSpec],
    sizes: &[SIZE],
    widths: &[i32],
    separator: COLORREF,
    inset: i32,
) {
    let mut x = 0;
    for (index, ((cell, size), width)) in cells.iter().zip(sizes).zip(widths).enumerate() {
        if index > 0 {
            view::fill_rect(
                hdc,
                RECT {
                    left: x,
                    top: client.top + inset,
                    right: x + 1,
                    bottom: client.bottom - inset,
                },
                separator,
            );
        }
        let ox = x + (width - size.cx) / 2;
        let oy = client.top + (client.bottom - client.top - size.cy) / 2;
        view::draw_text(hdc, cell.font, cell.color, ox, oy, &cell.text);
        x += width;
    }
}

impl Drop for StatusBar {
    fn drop(&mut self) {
        PLACEMENTS.with(|map| map.borrow_mut().remove(&(self.hwnd.0 as isize)));
        let _ = unsafe { DestroyWindow(self.hwnd) };
    }
}

/// 首次出现的位置：主显示器工作区右下角，留出边距与阴影。
fn default_anchor(content: (i32, i32), margin: i32) -> (i32, i32) {
    let work = monitor::primary_work_area();
    let gap = ((EDGE_GAP * margin) / 16).max(EDGE_GAP);
    (
        work.right - margin - gap - content.0,
        work.bottom - margin - gap - content.1,
    )
}

/// 把内容左上角夹进所在显示器的工作区，使整块内容可见。
fn clamp_anchor(anchor: (i32, i32), content: (i32, i32), margin: i32) -> (i32, i32) {
    let work = monitor::work_area_near(POINT {
        x: anchor.0,
        y: anchor.1,
    });
    let x = anchor.0.clamp(
        work.left + margin,
        (work.right - margin - content.0).max(work.left + margin),
    );
    let y = anchor.1.clamp(
        work.top + margin,
        (work.bottom - margin - content.1).max(work.top + margin),
    );
    (x, y)
}

fn placement_of(hwnd: HWND) -> Option<Rc<Placement>> {
    // clone 出来放开借用，再调回调。
    PLACEMENTS.with(|map| map.borrow().get(&(hwnd.0 as isize)).cloned())
}

/// 按下：拖动交给系统移动循环，没拖就是点击；点击不激活；拖动结束报位置。
unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_NCHITTEST => LRESULT(HTCLIENT as isize),
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_LBUTTONDOWN => {
            let mut point = POINT::default();
            let _ = unsafe { GetCursorPos(&mut point) };
            if unsafe { DragDetect(hwnd, point) }.as_bool() {
                let _ = unsafe { ReleaseCapture() };
                unsafe {
                    SendMessageW(
                        hwnd,
                        WM_NCLBUTTONDOWN,
                        Some(WPARAM(HTCAPTION as usize)),
                        Some(LPARAM(0)),
                    )
                };
            } else if let Some(placement) = placement_of(hwnd) {
                // 两个 16 位分量均为有符号客户区坐标，字标区域不触发功能格。
                placement.on_click(
                    (lparam.0 & 0xFFFF) as i16 as i32,
                    ((lparam.0 >> 16) & 0xFFFF) as i16 as i32,
                );
            }
            LRESULT(0)
        }
        WM_TIMER if wparam.0 == fullscreen::TIMER_ID => {
            if let Some(placement) = placement_of(hwnd) {
                fullscreen::on_timer(hwnd, &placement.fullscreen_hidden);
            }
            LRESULT(0)
        }
        WM_EXITSIZEMOVE => {
            if let Some(placement) = placement_of(hwnd) {
                placement.on_moved();
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
