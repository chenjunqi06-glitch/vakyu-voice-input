//! 隐藏的识别子进程与有界 JSONL 管道；退出或协议异常必定关闭子进程。

use std::io::{BufRead, BufReader, Read, Write};
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, SyncSender};

use super::command::HelperCommand;
use super::event::HelperEvent;
use qingjian_core::Language;

const MAX_EVENT_BYTES: u64 = 1_048_576;

pub(super) struct HelperProcess {
    child: Child,

    commands: SyncSender<HelperCommand>,

    pub events: Receiver<Result<HelperEvent, String>>,
}

impl HelperProcess {
    pub fn spawn(root: &Path) -> Result<Self, String> {
        let python = root.join("python/python.exe");
        let script = root.join("app/main.py");
        let models = root.join("models");
        if !python.is_file() || !script.is_file() || !models.is_dir() {
            return Err("中文语音组件不完整，请修复安装后再试".to_owned());
        }
        let mut child = Command::new(python)
            .arg("-u")
            .arg(script)
            .arg("--models")
            .arg(models)
            .current_dir(root)
            .env("PYTHONNOUSERSITE", "1")
            .env("PYTHONUTF8", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            // 识别正文不进入日志，第三方库也不能意外把它写入控制台。
            .stderr(Stdio::null())
            .creation_flags(0x0800_0000)
            .spawn()
            .map_err(|_| "无法启动中文语音组件，请修复安装后再试".to_owned())?;
        let mut stdin = child.stdin.take().expect("识别进程标准输入");
        let stdout = child.stdout.take().expect("识别进程标准输出");
        let (command_tx, command_rx) = mpsc::sync_channel::<HelperCommand>(8);
        let (event_tx, event_rx) = mpsc::sync_channel(32);
        let mut process = Self {
            child,
            commands: command_tx,
            events: event_rx,
        };
        let writer_events = event_tx.clone();
        std::thread::Builder::new()
            .name("miracle-voice-send".to_owned())
            .spawn(move || {
                while let Ok(command) = command_rx.recv() {
                    let result = serde_json::to_writer(&mut stdin, &command)
                        .map_err(std::io::Error::other)
                        .and_then(|()| stdin.write_all(b"\n"))
                        .and_then(|()| stdin.flush());
                    if result.is_err() {
                        let _ = writer_events.send(Err("中文语音控制连接已断开".to_owned()));
                        break;
                    }
                    if command.command == "shutdown" {
                        break;
                    }
                }
                // 关闭 stdin；父进程退出时 helper 必须立即关闭麦克风。
            })
            .map_err(|_| "无法建立中文语音控制线程".to_owned())?;
        std::thread::Builder::new()
            .name("miracle-voice-read".to_owned())
            .spawn(move || {
                let mut reader = BufReader::new(stdout);
                loop {
                    let event = read_event(&mut reader);
                    let failed = event.is_err();
                    if event_tx.send(event).is_err() || failed {
                        break;
                    }
                }
            })
            .map_err(|_| "无法建立中文语音结果线程".to_owned())?;
        // try_wait 也会清理刚启动就失败的进程；不等待模型加载。
        process.check_alive()?;
        Ok(process)
    }

    pub fn send(&self, command: &'static str, generation: u64) -> Result<(), String> {
        self.send_with_practice(command, generation, None, None)
    }

    pub fn send_with_practice(
        &self,
        command: &'static str,
        generation: u64,
        practice_text: Option<&str>,
        language: Option<Language>,
    ) -> Result<(), String> {
        self.commands
            .try_send(HelperCommand {
                command,
                generation,
                language: language.map(Language::code),
                practice_text: practice_text.map(str::to_owned),
            })
            .map_err(|_| "中文语音控制无响应，请重新按住 Alt 重试".to_owned())
    }

    pub fn check_alive(&mut self) -> Result<(), String> {
        match self.child.try_wait() {
            Ok(None) => Ok(()),
            _ => Err("中文语音组件意外退出，请重新按住 Alt 重试".to_owned()),
        }
    }
}

impl Drop for HelperProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub(super) fn read_event(reader: &mut impl BufRead) -> Result<HelperEvent, String> {
    let mut line = String::new();
    let length = reader
        .take(MAX_EVENT_BYTES + 1)
        .read_line(&mut line)
        .map_err(|_| "中文语音结果读取失败".to_owned())?;
    if length == 0 {
        return Err("中文语音结果连接已关闭".to_owned());
    }
    if length as u64 > MAX_EVENT_BYTES || !line.ends_with('\n') {
        return Err("中文语音结果格式异常".to_owned());
    }
    serde_json::from_str(&line).map_err(|_| "中文语音结果格式异常".to_owned())
}
