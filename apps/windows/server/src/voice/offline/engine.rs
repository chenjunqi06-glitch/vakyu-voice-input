//! 进程内唯一的离线翻译子进程：跟读与划词共用，第一次翻译时启动；子进程空闲 10 分钟自行退出，下次请求再启动。

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use qingjian_core::Language;

use super::reply::Reply;
use super::request::Request;

/// 单行结果上限（译文最多三万多字）。
const MAX_LINE_BYTES: u64 = 1_048_576;

type Routes = Arc<Mutex<HashMap<u64, Sender<Reply>>>>;

static ENGINE: Mutex<Option<Engine>> = Mutex::new(None);

static NEXT_WIRE: AtomicU64 = AtomicU64::new(1);

struct Engine {
    child: Child,

    stdin: ChildStdin,

    routes: Routes,

    /// 读线程看到子进程退出（空闲退出或崩溃）后置为 `false`。
    alive: Arc<AtomicBool>,
}

/// 提交一条翻译，返回请求号；子进程不在就先启动，写入失败时换新进程重试一次。
pub(super) fn submit(
    root: &Path,
    text: &str,
    target: Language,
    reply: Sender<Reply>,
) -> Result<u64, String> {
    let mut guard = ENGINE
        .lock()
        .map_err(|_| "离线翻译暂时不可用，请重试".to_owned())?;
    let wire = NEXT_WIRE.fetch_add(1, Ordering::Relaxed);
    let line = serde_json::to_string(&Request {
        command: "translate",
        id: wire,
        text,
        target: target.code(),
    })
    .map_err(|_| "离线翻译请求格式异常".to_owned())?;
    for attempt in 0..2 {
        if guard
            .as_ref()
            .is_none_or(|engine| !engine.alive.load(Ordering::Acquire))
        {
            *guard = Some(Engine::spawn(root)?);
        }
        let engine = guard.as_mut().expect("刚启动离线翻译进程");
        if let Ok(mut routes) = engine.routes.lock() {
            routes.insert(wire, reply.clone());
        }
        let written = engine
            .stdin
            .write_all(line.as_bytes())
            .and_then(|()| engine.stdin.write_all(b"\n"))
            .and_then(|()| engine.stdin.flush());
        if written.is_ok() {
            return Ok(wire);
        }
        if let Ok(mut routes) = engine.routes.lock() {
            routes.remove(&wire);
        }
        engine.alive.store(false, Ordering::Release);
        tracing::info!(attempt, "离线翻译进程已退出，重新启动");
    }
    Err("离线翻译进程无法启动，请重新安装 Vakyu 完整安装包".to_owned())
}

impl Engine {
    fn spawn(root: &Path) -> Result<Self, String> {
        let mut child = Command::new(root.join("python/pythonw.exe"))
            // 与识别进程相同：隔离系统 Python、不写 __pycache__、输出不缓冲。
            .arg("-I")
            .arg("-B")
            .arg("-u")
            .args(["-X", "utf8"])
            .arg(root.join("app/translate_main.py"))
            .arg("--models")
            .arg(root.join("models"))
            .current_dir(root)
            .env("PYTHONNOUSERSITE", "1")
            .env("PYTHONUTF8", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            // 原文与译文不进入日志。
            .stderr(Stdio::null())
            .creation_flags(0x0800_0000)
            .spawn()
            .map_err(|_| "无法启动离线翻译组件，请修复安装后再试".to_owned())?;
        let stdin = child.stdin.take().expect("离线翻译进程标准输入");
        let stdout = child.stdout.take().expect("离线翻译进程标准输出");
        let routes: Routes = Arc::new(Mutex::new(HashMap::new()));
        let alive = Arc::new(AtomicBool::new(true));
        let reader_routes = Arc::clone(&routes);
        let reader_alive = Arc::clone(&alive);
        std::thread::Builder::new()
            .name("vakyu-offline-translate".to_owned())
            .spawn(move || read_replies(BufReader::new(stdout), &reader_routes, &reader_alive))
            .map_err(|_| "无法建立离线翻译结果线程".to_owned())?;
        tracing::info!("离线翻译进程已启动");
        Ok(Self {
            child,
            stdin,
            routes,
            alive,
        })
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// 逐行读结果并按请求号转发；子进程退出时把还在等的请求都回成失败。
fn read_replies(mut reader: impl BufRead, routes: &Routes, alive: &AtomicBool) {
    loop {
        let mut line = String::new();
        let read = (&mut reader).take(MAX_LINE_BYTES + 1).read_line(&mut line);
        let Ok(length) = read else {
            break;
        };
        if length == 0 || length as u64 > MAX_LINE_BYTES {
            break;
        }
        let Some((wire, result)) = parse(&line) else {
            continue;
        };
        let sender = routes
            .lock()
            .ok()
            .and_then(|mut routes| routes.remove(&wire));
        if let Some(sender) = sender {
            let _ = sender.send(Reply { wire, result });
        }
    }
    alive.store(false, Ordering::Release);
    if let Ok(mut routes) = routes.lock() {
        for (wire, sender) in routes.drain() {
            let _ = sender.send(Reply {
                wire,
                result: Err("离线翻译进程已退出，请重试".to_owned()),
            });
        }
    }
}

/// 只认 `translation` 事件；`ready` 等其他行忽略。
pub(super) fn parse(line: &str) -> Option<(u64, Result<String, String>)> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    if value.get("event")?.as_str()? != "translation" {
        return None;
    }
    let wire = value.get("id")?.as_u64()?;
    let ok = value.get("ok").and_then(serde_json::Value::as_bool) == Some(true);
    let result = if ok {
        value
            .get("text")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| "离线翻译结果格式异常".to_owned())
    } else {
        Err(value
            .get("message")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("离线翻译失败，请重试")
            .to_owned())
    };
    Some((wire, result))
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn only_translation_events_are_routed() {
        assert_eq!(parse(r#"{"event":"ready"}"#), None);
        assert_eq!(
            parse(r#"{"event":"translation","id":3,"ok":true,"text":"Hello"}"#),
            Some((3, Ok("Hello".to_owned())))
        );
        assert_eq!(
            parse(r#"{"event":"translation","id":4,"ok":false,"message":"模型缺失"}"#),
            Some((4, Err("模型缺失".to_owned())))
        );
        assert_eq!(parse("garbage"), None);
    }
}
