//! Server 进程内的 UI 线程：候选窗口与悬浮状态条都在这条线程上自绘（普通置顶窗会被商店 / 任务栏搜索
//! 这些高 z-band 宿主盖住，只有本进程配合 uiAccess 签名才能盖过）。
//!
//! HWND 线程亲和：Router 在工人线程上产出内容，经通道 + `PostThreadMessageW` 唤醒交给 UI 线程应用。
//! 线程句柄是 [`UiHandle`]，命令在 [`command`]，候选窗口在 [`candidates`]，状态条在 [`status`]，分层窗口合成在 [`layered`]。

mod candidates;
mod command;
mod layered;
mod monitor;
mod painter;
mod selection;
mod status;
mod tray;
mod voice;
mod window_class;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use windows::Win32::Foundation::{E_FAIL, HINSTANCE, LPARAM, RECT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::WindowsAndMessaging::{
    ASFW_ANY, AllowSetForegroundWindow, DispatchMessageW, GetMessageW, MSG, PostThreadMessageW,
    TranslateMessage, WM_APP,
};
use windows::core::{Error, Result};

use qingjian_platform::protocol::{Frame, ScreenRect};

use self::candidates::CandidateWindow;
use self::command::UiCommand;
use self::painter::{Painter, SharedPainter};
use self::selection::SelectionTranslate;
use self::status::StatusBar;
use self::tray::TrayIcon;
use self::voice::VoiceWindow;
use crate::dispatch::{
    CandidateSink, PracticeView, RecordingMode, RenderSettings, StatusEvent, StatusSink,
    StatusView, TrayState, VoiceSink,
};

/// 状态条上的操作（点格子 / 拖动结束）回给 Router 的回调，UI 线程上调。
pub type StatusEvents = Arc<dyn Fn(StatusEvent) + Send + Sync>;

/// 唤醒 UI 线程去排空命令队列的线程消息。
const WM_WAKE: u32 = WM_APP;

/// UI 线程句柄：发命令 = 投进通道 + 一条 `WM_WAKE`。线程 detached，随进程存活。
#[derive(Clone)]
pub struct UiHandle {
    /// 命令通道的发送端。
    sender: Sender<UiCommand>,

    /// UI 线程 id，`PostThreadMessageW` 用。
    thread_id: u32,
}

impl UiHandle {
    /// 起 UI 线程并等它建好候选窗口。失败返回 `Err`，调用方退化为不画。
    pub fn spawn(on_status: StatusEvents) -> Result<Self> {
        // 用 Option<u32> 而非 Result 回报，免得 windows Error 跨线程。
        let (ready_tx, ready_rx) = mpsc::channel::<Option<u32>>();
        let (command_tx, command_rx) = mpsc::channel::<UiCommand>();
        thread::Builder::new()
            .name("miracle-candidates".to_owned())
            .spawn(move || run(command_rx, &ready_tx, on_status))
            .map_err(|_| Error::from(E_FAIL))?;
        match ready_rx.recv() {
            Ok(Some(thread_id)) => Ok(Self {
                sender: command_tx,
                thread_id,
            }),
            _ => Err(Error::from(E_FAIL)),
        }
    }

    /// 线程已退出（通道断）时静默丢弃。
    fn post(&self, command: UiCommand) {
        if self.sender.send(command).is_ok() {
            let _ = unsafe { PostThreadMessageW(self.thread_id, WM_WAKE, WPARAM(0), LPARAM(0)) };
        }
    }
}

impl VoiceSink for UiHandle {
    fn set_voice_level(&self, level: u8) {
        self.post(UiCommand::VoiceLevel(level.min(100)));
    }

    fn set_voice_held(&self, held: bool) {
        self.post(UiCommand::VoiceHeld(held));
    }

    fn voice_error(&self, message: String, rect: ScreenRect, mode: RecordingMode) {
        self.post(UiCommand::VoiceError(Box::new((message, rect, mode))));
    }

    fn prepare_voice(&self, rect: ScreenRect, mode: RecordingMode) {
        self.post(UiCommand::VoicePrepare(rect, mode));
    }

    fn show_voice(&self, text: String, rect: ScreenRect, mode: RecordingMode) {
        self.post(UiCommand::VoiceShow(Box::new((text, rect, mode))));
    }

    fn show_practice(&self, view: PracticeView, rect: ScreenRect) {
        self.post(UiCommand::VoicePractice(Box::new((view, rect))));
    }

    fn hide_voice(&self) {
        self.post(UiCommand::VoiceHide);
    }
}

impl CandidateSink for UiHandle {
    fn show(&self, frame: Frame, rect: ScreenRect) {
        self.post(UiCommand::Show(Box::new((frame, rect))));
    }

    fn hide(&self) {
        self.post(UiCommand::Hide);
    }

    fn configure(&self, settings: RenderSettings) {
        self.post(UiCommand::Configure(settings));
    }
}

impl StatusSink for UiHandle {
    fn show_status(&self, view: StatusView) {
        self.post(UiCommand::StatusShow(Box::new(view)));
    }

    fn hide_status(&self) {
        self.post(UiCommand::StatusHide);
    }

    fn configure_tray(&self, state: TrayState) {
        self.post(UiCommand::TrayConfigure(state));
    }

    fn configure_selection(&self, enabled: bool) {
        self.post(UiCommand::SelectionConfigure(enabled));
    }

    fn open_settings(&self) {
        open_settings();
    }

    fn open_download(&self) {
        // 旧菜单消息也不能将 Vakyu 用户带到不兼容的上游安装包。
        open_settings();
    }
}

/// 起与本 exe 同目录的设置程序。设置程序已开时由新实例把它带到前台，得先把前台权让出去。
pub(crate) fn open_settings() {
    let _ = unsafe { AllowSetForegroundWindow(ASFW_ANY) };
    let exe = std::env::current_exe().map(|exe| exe.with_file_name("qingjian-settings.exe"));
    let spawned = exe.and_then(|exe| std::process::Command::new(exe).spawn());
    if let Err(error) = spawned {
        tracing::warn!(%error, "打开设置程序失败");
    }
}

/// 本进程 exe 的模块句柄（注册窗口类 / 建窗口用）。
pub(super) fn module_handle() -> HINSTANCE {
    let module = unsafe { GetModuleHandleW(None) }.unwrap_or_default();
    HINSTANCE(module.0)
}

/// UI 线程主体：建窗口、报回线程 id、跑消息循环。
fn run(commands: Receiver<UiCommand>, ready: &Sender<Option<u32>>, on_status: StatusEvents) {
    // 按物理像素定位，与应用报来的组句屏幕矩形对齐；已设过会失败，忽略。
    let _ = unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    let thread_id = unsafe { GetCurrentThreadId() };
    // 装上时随 Configure 命令建。
    let painter: SharedPainter = Rc::new(RefCell::new(None));
    // 先建窗口再报 id：建窗口顺带建起本线程的消息队列，之后 PostThreadMessageW 才有处可投。
    let window = match CandidateWindow::new(painter.clone()) {
        Ok(window) => window,
        Err(error) => {
            tracing::error!(%error, "建候选窗口失败，Server 将不显示候选框");
            let _ = ready.send(None);
            return;
        }
    };
    let status = match StatusBar::new(on_status.clone(), painter.clone()) {
        Ok(status) => Some(status),
        Err(error) => {
            tracing::error!(%error, "建悬浮状态条失败，将不显示状态条");
            None
        }
    };
    let tray = match TrayIcon::new(on_status.clone()) {
        Ok(tray) => Some(tray),
        Err(error) => {
            tracing::error!(%error, "建托盘图标失败");
            None
        }
    };
    let voice = match VoiceWindow::new(on_status.clone()) {
        Ok(voice) => Some(voice),
        Err(error) => {
            tracing::error!(%error, "建语音浮窗失败，将不显示语音状态");
            None
        }
    };
    let selection = match SelectionTranslate::new() {
        Ok(selection) => Some(selection),
        Err(error) => {
            tracing::error!(%error, "建划词翻译窗口失败，将不显示「译」图标");
            None
        }
    };
    let ui = Ui {
        window: &window,
        status: status.as_ref(),
        tray: tray.as_deref(),
        voice: voice.as_ref(),
        selection: selection.as_ref(),
        painter: &painter,
    };
    if ready.send(Some(thread_id)).is_err() {
        return;
    }
    let mut msg = MSG::default();
    loop {
        let got = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        if got.0 <= 0 {
            break;
        }
        if msg.message == WM_WAKE {
            // 一次唤醒排空整个队列。连续输入会在很短时间内排入多帧候选，
            // 这些中间帧已经过时；只保留相邻 Show 中的最后一帧，避免 UI
            // 线程把积压的整张位图逐帧测量、光栅化和提交。
            let mut pending_show = None;
            while let Ok(command) = commands.try_recv() {
                match command {
                    UiCommand::Show(payload) => pending_show = Some(payload),
                    other => {
                        if let Some(payload) = pending_show.take() {
                            apply(&ui, UiCommand::Show(payload));
                        }
                        apply(&ui, other);
                    }
                }
            }
            if let Some(payload) = pending_show {
                apply(&ui, UiCommand::Show(payload));
            }
            continue;
        }
        unsafe {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

/// UI 线程上各窗口的借用，命令按类型分给它们。
#[derive(Clone, Copy)]
struct Ui<'a> {
    window: &'a CandidateWindow,

    status: Option<&'a StatusBar>,

    tray: Option<&'a TrayIcon>,

    voice: Option<&'a VoiceWindow>,

    selection: Option<&'a SelectionTranslate>,

    painter: &'a SharedPainter,
}

fn apply(ui: &Ui<'_>, command: UiCommand) {
    let Ui {
        window,
        status,
        tray,
        voice,
        selection,
        painter,
    } = *ui;
    match command {
        UiCommand::Show(payload) => {
            let (frame, rect) = *payload;
            window.set_content(&frame);
            window.show(to_win_rect(rect));
        }
        UiCommand::Hide => window.hide(),
        UiCommand::VoiceHeld(held) => {
            if let Some(voice) = voice {
                voice.set_held(held);
            }
        }
        UiCommand::VoiceLevel(level) => {
            if let Some(voice) = voice {
                voice.set_level(level);
            }
        }
        UiCommand::VoiceShow(payload) => {
            if let Some(voice) = voice {
                let (text, rect, mode) = *payload;
                voice.show(text, to_win_rect(rect), "正在聆听", true, mode);
            }
        }
        UiCommand::VoicePractice(payload) => {
            if let Some(voice) = voice {
                let (view, rect) = *payload;
                voice.show_practice(&view, to_win_rect(rect));
            }
        }
        UiCommand::VoicePrepare(rect, mode) => {
            if let Some(voice) = voice {
                voice.show(
                    "准备就绪后再开始说话…".to_owned(),
                    to_win_rect(rect),
                    "正在准备麦克风",
                    false,
                    mode,
                );
            }
        }
        UiCommand::VoiceError(payload) => {
            if let Some(voice) = voice {
                let (message, rect, mode) = *payload;
                voice.show_error(message, to_win_rect(rect), mode);
            }
        }
        UiCommand::VoiceHide => {
            if let Some(voice) = voice {
                voice.hide();
            }
        }
        UiCommand::StatusShow(view) => {
            if let Some(status) = status {
                status.update(*view);
            }
        }
        UiCommand::StatusHide => {
            if let Some(status) = status {
                status.hide();
            }
        }
        UiCommand::TrayConfigure(state) => {
            if let Some(tray) = tray {
                tray.configure(state);
            }
        }
        UiCommand::Configure(settings) => Painter::configure(painter, &settings),
        UiCommand::SelectionConfigure(enabled) => {
            if let Some(selection) = selection {
                selection.configure(enabled);
            }
        }
    }
}

fn to_win_rect(rect: ScreenRect) -> RECT {
    RECT {
        left: rect.left,
        top: rect.top,
        right: rect.right,
        bottom: rect.bottom,
    }
}
