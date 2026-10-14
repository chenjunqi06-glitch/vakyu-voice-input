//! 一个朗读子进程：随包 Python 运行 `app/speak_main.py`，读线程把事件转进通道；子进程退出时通道断开。

use std::io::{BufRead, BufReader, Read, Write};
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::time::{Duration, Instant};

use super::event::Event;

/// 一行事件的上限；正常事件只有几十字节。
const MAX_LINE_BYTES: u64 = 64 * 1024;

pub(super) struct Process {
    child: Child,

    stdin: ChildStdin,

    events: Receiver<Event>,
}

impl Process {
    /// 启动并等到模型加载完（`ready`）；第一次要从硬盘读约 80MB，慢硬盘也留足时间。
    pub fn start(root: &Path, timeout: Duration) -> Result<Self, String> {
        let mut child = Command::new(root.join("python/pythonw.exe"))
            // 与识别进程相同：隔离系统 Python、不写 __pycache__、输出不缓冲。
            .arg("-I")
            .arg("-B")
            .arg("-u")
            .args(["-X", "utf8"])
            .arg(root.join("app/speak_main.py"))
            .arg("--models")
            .arg(root.join("models"))
            .current_dir(root)
            .env("PYTHONNOUSERSITE", "1")
            .env("PYTHONUTF8", "1")
            .env("OMP_NUM_THREADS", "2")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            // 朗读的句子不进入日志。
            .stderr(Stdio::null())
            .creation_flags(0x0800_0000)
            .spawn()
            .map_err(|_| "无法启动自然人声组件".to_owned())?;
        let stdin = child.stdin.take().expect("朗读进程标准输入");
        let stdout = child.stdout.take().expect("朗读进程标准输出");
        let (sender, events) = mpsc::channel();
        if std::thread::Builder::new()
            .name("vakyu-natural-voice".to_owned())
            .spawn(move || read_events(BufReader::new(stdout), &sender))
            .is_err()
        {
            let _ = child.kill();
            let _ = child.wait();
            return Err("无法建立自然人声结果线程".to_owned());
        }
        let mut process = Self {
            child,
            stdin,
            events,
        };
        process.wait_ready(timeout)?;
        tracing::info!("自然人声进程已启动");
        Ok(process)
    }

    fn wait_ready(&mut self, timeout: Duration) -> Result<(), String> {
        let deadline = Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            match self.events.recv_timeout(remaining) {
                Ok(Event::Ready) => return Ok(()),
                Ok(Event::Fatal(message)) => return Err(message),
                Ok(_) => {}
                Err(RecvTimeoutError::Timeout) => {
                    return Err("自然人声加载超时".to_owned());
                }
                Err(RecvTimeoutError::Disconnected) => {
                    return Err("自然人声组件启动后立即退出".to_owned());
                }
            }
        }
    }

    /// 空闲 10 分钟子进程会自行退出；下次朗读前据此重启。
    pub fn alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    pub fn send(&mut self, line: &str) -> std::io::Result<()> {
        self.stdin.write_all(line.as_bytes())?;
        self.stdin.write_all(b"\n")?;
        self.stdin.flush()
    }

    /// 取一条事件；`Err(())` 表示子进程已经退出。
    pub fn next_event(&self) -> Result<Option<Event>, ()> {
        match self.events.try_recv() {
            Ok(event) => Ok(Some(event)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(()),
        }
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.send(r#"{"command":"shutdown"}"#);
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn read_events(mut reader: impl BufRead, events: &Sender<Event>) {
    loop {
        let mut line = String::new();
        let read = (&mut reader).take(MAX_LINE_BYTES + 1).read_line(&mut line);
        let Ok(length) = read else {
            return;
        };
        if length == 0 || length as u64 > MAX_LINE_BYTES {
            return;
        }
        if let Some(event) = Event::parse(&line)
            && events.send(event).is_err()
        {
            return;
        }
    }
}
