//! 本机 ASR 子进程；有界管道和关闭时的进程回收，禁止积压无限音频。

use crate::{arguments::Arguments, protocol::AsrEvent};
use serde_json::Value;
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver, SyncSender},
    thread,
};

pub struct Runtime {
    child: Child,

    input: SyncSender<Value>,

    output: Receiver<AsrEvent>,
}

impl Runtime {
    pub fn start(args: &Arguments) -> std::io::Result<Self> {
        let mut child = Command::new(&args.python)
            .arg(args.runtime.join("external_main.py"))
            .arg("--models")
            .arg(&args.models)
            .env("PYTHONUNBUFFERED", "1")
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .env("PYTHONNOUSERSITE", "1")
            .env_remove("PYTHONPATH")
            .env_remove("PYTHONHOME")
            .env("OMP_NUM_THREADS", "2")
            .env("OPENBLAS_NUM_THREADS", "2")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let mut stdin = child.stdin.take().expect("子进程标准输入");
        let stdout = child.stdout.take().expect("子进程标准输出");
        let (input, incoming) = mpsc::sync_channel::<Value>(128);
        let (outgoing, output) = mpsc::sync_channel(128);
        thread::spawn(move || {
            for command in incoming {
                if serde_json::to_writer(&mut stdin, &command).is_err()
                    || stdin.write_all(b"\n").is_err()
                    || stdin.flush().is_err()
                {
                    break;
                }
            }
        });
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else {
                    break;
                };
                if line.len() > 1_048_576 {
                    break;
                }
                let Ok(event) = serde_json::from_str::<AsrEvent>(&line) else {
                    continue;
                };
                if outgoing.send(event).is_err() {
                    return;
                }
            }
        });
        Ok(Self {
            child,
            input,
            output,
        })
    }

    pub fn send(&self, command: Value) -> bool {
        self.input.try_send(command).is_ok()
    }

    pub fn poll(&self) -> Option<AsrEvent> {
        self.output.try_recv().ok()
    }

    pub fn exited(&mut self) -> bool {
        !matches!(self.child.try_wait(), Ok(None))
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
