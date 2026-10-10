//! 常驻识别进程的生命周期；所有等待都在语音线程，不阻塞输入法 IPC。

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use qingjian_core::Language;
use qingjian_platform::protocol::VoiceUpdate;

use super::closing::CloseWatchdog;
use super::event::HelperEvent;
use super::process::HelperProcess;
use super::session::{FINISH_TIMEOUT, Session};
use crate::voice::command::Command;
use crate::voice::meter::AudioMeter;
use crate::voice::start::requested;

const POLL_INTERVAL: Duration = Duration::from_millis(20);
const WARMUP_TIMEOUT: Duration = Duration::from_secs(60);

pub(in crate::voice) fn run(
    root: PathBuf,
    commands: Receiver<Command>,
    updates: Sender<(u64, VoiceUpdate)>,
    desired: Arc<AtomicU64>,
    meter: AudioMeter,
) {
    let mut active = None;
    loop {
        let result = HelperProcess::spawn(&root).and_then(|mut helper| {
            tracing::info!("中英语音组件已启动，正在预热");
            run_connected(
                &mut helper,
                &commands,
                &updates,
                &desired,
                &mut active,
                &meter,
            )
        });
        let Err(message) = result else {
            return;
        };
        tracing::warn!(%message, "中英语音组件不可用");
        if let Some(session) = active.take() {
            send(
                &updates,
                session.generation,
                VoiceUpdate::Error {
                    message: message.clone(),
                },
            );
        }
        // 失败后仅下一次明确按住才能重新加载；后台重试绝不能意外开麦。
        let retry_after = Instant::now() + Duration::from_secs(2);
        loop {
            let Ok(command) = commands.recv() else {
                return;
            };
            match command.with_language_defaults() {
                Command::StartInLanguage(generation, language) => {
                    match requested(&desired, generation) {
                        Some(true) if Instant::now() >= retry_after => {
                            active = Some(Session::in_language(generation, language));
                            break;
                        }
                        Some(true) => send(
                            &updates,
                            generation,
                            VoiceUpdate::Error {
                                message: message.clone(),
                            },
                        ),
                        Some(false) => send_empty(&updates, generation),
                        None => {}
                    }
                }
                Command::StartPracticeInLanguage(generation, text, language) => {
                    match requested(&desired, generation) {
                        Some(true) if Instant::now() >= retry_after => {
                            active =
                                Some(Session::practice_in_language(generation, text, language));
                            break;
                        }
                        Some(true) => send(
                            &updates,
                            generation,
                            VoiceUpdate::Error {
                                message: message.clone(),
                            },
                        ),
                        Some(false) => send_empty(&updates, generation),
                        None => {}
                    }
                }
                Command::Shutdown => return,
                _ => {}
            }
        }
    }
}

fn run_connected(
    helper: &mut HelperProcess,
    commands: &Receiver<Command>,
    updates: &Sender<(u64, VoiceUpdate)>,
    desired: &AtomicU64,
    active: &mut Option<Session>,
    meter: &AudioMeter,
) -> Result<(), String> {
    let warmup_deadline = Instant::now() + WARMUP_TIMEOUT;
    let mut ready = false;
    let mut closing = CloseWatchdog::default();
    loop {
        match commands
            .recv_timeout(POLL_INTERVAL)
            .map(Command::with_language_defaults)
        {
            Ok(Command::Shutdown) | Err(RecvTimeoutError::Disconnected) => {
                let _ = helper.send("shutdown", 0);
                return Ok(());
            }
            Ok(Command::Cancel) => cancel(
                &|command, generation| helper.send(command, generation),
                active,
                &mut closing,
            )?,
            Ok(Command::StartInLanguage(generation, language)) => {
                cancel(
                    &|command, generation| helper.send(command, generation),
                    active,
                    &mut closing,
                )?;
                match requested(desired, generation) {
                    Some(true) => *active = Some(Session::in_language(generation, language)),
                    Some(false) => send_empty(updates, generation),
                    None => {}
                }
            }
            Ok(Command::StartPracticeInLanguage(generation, text, language)) => {
                cancel(
                    &|command, generation| helper.send(command, generation),
                    active,
                    &mut closing,
                )?;
                match requested(desired, generation) {
                    Some(true) => {
                        *active = Some(Session::practice_in_language(generation, text, language))
                    }
                    Some(false) => send_empty(updates, generation),
                    None => {}
                }
            }
            Ok(Command::Stop(generation)) => {
                if active
                    .as_ref()
                    .is_some_and(|session| session.generation == generation)
                {
                    stop(
                        &|command, generation| helper.send(command, generation),
                        active,
                        updates,
                        &mut closing,
                    )?;
                }
            }
            Ok(Command::Start(_) | Command::StartPractice(..)) => unreachable!("已补全识别语言"),
            Err(RecvTimeoutError::Timeout) => {}
        }
        // 原子意图先于消息队列生效；预热结束与松键同时发生时不能补开麦克风。
        reconcile_with_practice(
            &|command, generation, practice_text, language| {
                helper.send_with_practice(command, generation, practice_text, language)
            },
            active,
            updates,
            desired,
            ready,
            &mut closing,
        )?;
        // 一轮只处理有限事件，异常密集的输出也不能饿死停止命令和看门狗。
        for _ in 0..64 {
            let Ok(event) = helper.events.try_recv() else {
                break;
            };
            let event = event?;
            if closing.acknowledge(&event) {
                continue;
            }
            if matches!(event, HelperEvent::Ready) {
                if !ready {
                    ready = true;
                    tracing::info!("中英语音已就绪");
                }
                continue;
            }
            if let HelperEvent::Error {
                generation: None,
                message,
            } = event
            {
                return Err(message);
            }
            let Some(session) = active.as_mut().filter(|session| session.accepts(&event)) else {
                continue;
            };
            let generation = session.generation;
            if requested(desired, generation).is_none() {
                continue;
            }
            let update = match event {
                HelperEvent::Level { level, .. } => {
                    if requested(desired, generation) == Some(true) {
                        meter.update(generation, level);
                    }
                    continue;
                }
                HelperEvent::Started { .. } => {
                    session.started = true;
                    if session.finishing.is_some() || requested(desired, generation) != Some(true) {
                        continue;
                    }
                    VoiceUpdate::Started
                }
                HelperEvent::Interim { text, .. } => VoiceUpdate::Interim { text },
                HelperEvent::Final { text, .. } => {
                    *active = None;
                    VoiceUpdate::Final { text }
                }
                HelperEvent::Error { message, .. } => {
                    // 即使 helper 的错误路径忘了关麦，取消命令也会回收该代次。
                    helper.send("cancel", generation)?;
                    closing.arm(generation, true, Instant::now());
                    *active = None;
                    VoiceUpdate::Error { message }
                }
                HelperEvent::Ready
                | HelperEvent::Stopped { .. }
                | HelperEvent::Cancelled { .. } => unreachable!(),
            };
            send(updates, generation, update);
        }
        reconcile_with_practice(
            &|command, generation, practice_text, language| {
                helper.send_with_practice(command, generation, practice_text, language)
            },
            active,
            updates,
            desired,
            ready,
            &mut closing,
        )?;
        helper.check_alive()?;
        // 会话已取消时 active 为 None，仍须检查关闭确认，避免留下后台录音。
        closing.check(Instant::now())?;
        if !ready && Instant::now() >= warmup_deadline {
            return Err("中英语音模型加载超时，请修复安装后再试".to_owned());
        }
        if let Some(message) = active
            .as_ref()
            .and_then(|session| session.timeout(Instant::now()))
        {
            // 控制或结束超时意味着子进程可能仍持有麦克风，返回后直接销毁它。
            return Err(message.to_owned());
        }
    }
}

#[cfg(test)]
pub(super) fn reconcile(
    send_command: &impl Fn(&'static str, u64) -> Result<(), String>,
    active: &mut Option<Session>,
    updates: &Sender<(u64, VoiceUpdate)>,
    desired: &AtomicU64,
    ready: bool,
    closing: &mut CloseWatchdog,
) -> Result<(), String> {
    reconcile_with_practice(
        &|command, generation, _, _| send_command(command, generation),
        active,
        updates,
        desired,
        ready,
        closing,
    )
}

pub(super) fn reconcile_with_practice(
    send_command: &impl Fn(&'static str, u64, Option<&str>, Option<Language>) -> Result<(), String>,
    active: &mut Option<Session>,
    updates: &Sender<(u64, VoiceUpdate)>,
    desired: &AtomicU64,
    ready: bool,
    closing: &mut CloseWatchdog,
) -> Result<(), String> {
    let Some(session) = active.as_mut() else {
        return Ok(());
    };
    match requested(desired, session.generation) {
        None => cancel(
            &|command, generation| send_command(command, generation, None, None),
            active,
            closing,
        ),
        Some(false) => stop(
            &|command, generation| send_command(command, generation, None, None),
            active,
            updates,
            closing,
        ),
        Some(true) => {
            if ready && session.dispatched.is_none() {
                send_command(
                    "start",
                    session.generation,
                    session.practice_text.as_deref(),
                    Some(session.language),
                )?;
                session.dispatched = Some(Instant::now());
            }
            Ok(())
        }
    }
}

fn cancel(
    send_command: &impl Fn(&'static str, u64) -> Result<(), String>,
    active: &mut Option<Session>,
    closing: &mut CloseWatchdog,
) -> Result<(), String> {
    if let Some(session) = active.as_ref()
        && session.dispatched.is_some()
    {
        send_command("cancel", session.generation)?;
        closing.arm(session.generation, true, Instant::now());
    }
    *active = None;
    Ok(())
}

fn stop(
    send_command: &impl Fn(&'static str, u64) -> Result<(), String>,
    active: &mut Option<Session>,
    updates: &Sender<(u64, VoiceUpdate)>,
    closing: &mut CloseWatchdog,
) -> Result<(), String> {
    let Some(session) = active.as_mut() else {
        return Ok(());
    };
    if session.dispatched.is_none() {
        send_empty(updates, session.generation);
        *active = None;
    } else if session.finishing.is_none() {
        send_command("stop", session.generation)?;
        closing.arm(session.generation, false, Instant::now());
        session.finishing = Some(Instant::now() + FINISH_TIMEOUT);
    }
    Ok(())
}

fn send_empty(updates: &Sender<(u64, VoiceUpdate)>, generation: u64) {
    send(
        updates,
        generation,
        VoiceUpdate::Final {
            text: String::new(),
        },
    );
}

fn send(updates: &Sender<(u64, VoiceUpdate)>, generation: u64, update: VoiceUpdate) {
    let _ = updates.send((generation, update));
}
