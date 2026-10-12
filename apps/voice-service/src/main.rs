//! 原生宿主与平台无关跟读逻辑之间的轻量后台；stdout 只使用 JSON。

mod arguments;
mod configuration;
mod controller;
mod game;
mod protocol;
mod runtime;

use arguments::Arguments;
use clap::Parser;
use controller::Controller;
use protocol::{AsrEvent, Command, Effect, Event};
use qingjian_platform::Config;
use runtime::Runtime;
use std::{
    io::{self, BufRead, Write},
    sync::mpsc,
    time::{Duration, Instant},
};

fn output(event: &Event) -> io::Result<()> {
    let stdout = io::stdout();
    let mut lock = stdout.lock();
    serde_json::to_writer(&mut lock, event)?;
    lock.write_all(b"\n")?;
    lock.flush()
}

fn main() {
    if run().is_err() {
        let _ = output(&Event::Error { message: "语音后台无法运行，请检查本地语音组件 / Voice service is unavailable; check local speech components".to_owned() });
        std::process::exit(1);
    }
}

fn run() -> io::Result<()> {
    let args = Arguments::parse();
    std::fs::create_dir_all(&args.data)?;
    if let Some(parent) = args.config.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if !args.config.exists() {
        Config::write_template_if_missing(&args.config)
            .map_err(|_| io::Error::other("configuration unavailable"))?;
        Config::set_value(
            &args.config,
            "shortcut",
            "voice_practice_shortcut",
            "right_ctrl+right_alt",
        )
        .map_err(|_| io::Error::other("configuration unavailable"))?;
        Config::set_value(&args.config, "shortcut", "voice_key", "left_alt")
            .map_err(|_| io::Error::other("configuration unavailable"))?;
    }
    let config = configuration::load(&args.config)?;
    let mut controller = Controller::new(config, args.config.clone(), args.data.clone());
    let mut runtime = Runtime::start(&args)?;
    let (sender, receiver) = mpsc::sync_channel(128);
    std::thread::spawn(move || {
        for line in io::stdin().lock().lines() {
            let Ok(line) = line else {
                break;
            };
            if line.len() > 1_048_576 {
                break;
            }
            if let Ok(command) = serde_json::from_str::<Command>(&line)
                && sender.send(command).is_err()
            {
                return;
            }
        }
        let _ = sender.send(Command::Shutdown);
    });
    let mut stamp = configuration::stamp(&args.config);
    let mut checked = Instant::now();
    let started = Instant::now();
    let mut ready = false;
    let mut running = true;
    while running {
        for effect in controller.drain() {
            match effect {
                Effect::Native(event) => output(&event)?,
                Effect::Asr(command) => {
                    if !runtime.send(command) {
                        controller.fail(
                            "语音队列暂时繁忙，请重试 / Audio processing is busy; please retry"
                                .to_owned(),
                        );
                        // 不丢采样后继续假装完整识别，终止本轮并重启后台。
                        for effect in controller.drain() {
                            if let Effect::Native(event) = effect {
                                output(&event)?;
                            }
                        }
                        return Err(io::Error::other("recognizer backpressure"));
                    }
                }
            }
        }
        match receiver.recv_timeout(controller.poll_interval()) {
            Ok(command) => running = controller.command(command),
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        for _ in 0..128 {
            let Some(event) = runtime.poll() else {
                break;
            };
            if matches!(event, AsrEvent::Ready) {
                ready = true;
            }
            controller.backend(event);
        }
        let now = Instant::now();
        controller.tick(now);
        if now.duration_since(checked) > Duration::from_secs(1) {
            checked = now;
            if runtime.exited()
                || (!ready && now.duration_since(started) > Duration::from_secs(180))
            {
                return Err(io::Error::other("recognizer unavailable"));
            }
            let next = configuration::stamp(&args.config);
            if next != stamp {
                stamp = next;
                controller.reload();
            }
        }
    }
    for effect in controller.drain() {
        if let Effect::Native(event) = effect {
            output(&event)?;
        }
    }
    Ok(())
}
