//! 全局按住键线程的所有权句柄；配置和事件均走队列。

use super::{command::Command, config::TriggerConfig, event::TriggerEvent, native, shared::Shared};
use crate::voice::global::trigger::ActivitySource;
use std::sync::{
    Arc,
    atomic::Ordering,
    mpsc::{self, Receiver, Sender},
};
use std::thread::JoinHandle;
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::{
    Input::KeyboardAndMouse::GetAsyncKeyState,
    WindowsAndMessaging::{PostThreadMessageW, WM_QUIT},
};

pub struct HookThread {
    commands: Sender<Command>,

    events: Receiver<TriggerEvent>,

    shared: Arc<Shared>,

    thread_id: u32,

    worker: Option<JoinHandle<()>>,
}

impl HookThread {
    pub fn start(config: TriggerConfig) -> Result<Self, String> {
        let (commands, config_rx) = mpsc::channel();
        let (event_tx, events) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let shared = Arc::new(Shared::new());
        let thread_shared = Arc::clone(&shared);
        let worker = std::thread::Builder::new()
            .name("miracle-global-voice-key".into())
            .spawn(move || native::run(config, config_rx, event_tx, thread_shared, ready_tx))
            .map_err(|error| format!("无法启动全局语音按键线程：{error}"))?;
        match ready_rx.recv() {
            Ok(Ok(thread_id)) => Ok(Self {
                commands,
                events,
                shared,
                thread_id,
                worker: Some(worker),
            }),
            result => {
                let _ = worker.join();
                Err(match result {
                    Ok(Err(message)) => message,
                    _ => "全局语音按键线程意外退出".to_owned(),
                })
            }
        }
    }

    pub fn configure(&self, config: TriggerConfig) -> Result<(), String> {
        let (reply, result) = mpsc::channel();
        self.commands
            .send(Command::Configure(config, reply))
            .map_err(|_| "全局语音按键线程已退出".to_owned())?;
        self.wake()?;
        result
            .recv_timeout(std::time::Duration::from_secs(2))
            .map_err(|_| "全局语音按键配置未及时确认".to_owned())?
    }

    pub fn save_shortcut(
        &self,
        shortcut: qingjian_platform::PracticeShortcut,
        path: std::path::PathBuf,
    ) -> Result<(), String> {
        let (reply, result) = mpsc::channel();
        let timeout = std::time::Duration::from_secs(2);
        let deadline = std::time::Instant::now() + timeout;
        self.commands
            .send(Command::SaveShortcut(shortcut, path, reply, deadline))
            .map_err(|_| "全局语音按键线程已退出".to_owned())?;
        self.wake()?;
        result
            .recv_timeout(timeout)
            .map_err(|_| "快捷键保存未及时确认，原快捷键保持不变，请稍后重试".to_owned())?
    }

    fn wake(&self) -> Result<(), String> {
        unsafe { PostThreadMessageW(self.thread_id, native::CONFIG_MESSAGE, WPARAM(0), LPARAM(0)) }
            .map_err(|error| format!("无法更新全局语音按键配置：{error}"))
    }

    pub fn try_recv(&self) -> Option<TriggerEvent> {
        self.events.try_recv().ok()
    }

    pub fn activity_revision(&self) -> u64 {
        self.shared.revision.load(Ordering::Acquire)
    }

    /// 仅供取消日志诊断；读取时可能已发生下一次活动，不能用它代替代次保护。
    pub fn activity_source(&self) -> ActivitySource {
        ActivitySource::from_raw(self.shared.activity_source.load(Ordering::Acquire))
    }

    pub fn set_practice_active(&self, active: bool) {
        self.shared.practice_escape.store(false, Ordering::Release);
        self.shared.practice_active.store(active, Ordering::Release);
    }

    pub fn take_practice_escape(&self) -> bool {
        self.shared.practice_escape.swap(false, Ordering::AcqRel)
    }

    /// 只确认低层钩子观察到的按住轮次；不受系统 Alt 被暂扣的键盘状态影响。
    pub fn is_held(&self, sequence: u64) -> bool {
        sequence != 0 && self.healthy() && self.shared.sequence.load(Ordering::Acquire) == sequence
    }

    /// 消息泵失去响应或线程退出时立即禁止继续录音；不声称能查询系统是否静默移除了钩子。
    pub fn healthy(&self) -> bool {
        self.shared.alive.load(Ordering::Acquire)
            && self
                .shared
                .elapsed_ms()
                .saturating_sub(self.shared.heartbeat.load(Ordering::Acquire))
                < 1000
    }

    /// 仅用于诊断；被暂扣的 Alt 在不同 Windows 版本上的异步键盘状态可能不同。
    #[allow(dead_code)]
    pub fn physical_key_down(&self, sequence: u64) -> bool {
        self.is_held(sequence)
            && unsafe { GetAsyncKeyState(i32::from(self.shared.key.load(Ordering::Acquire))) < 0 }
    }
}

impl Drop for HookThread {
    fn drop(&mut self) {
        unsafe {
            let _ = PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
