//! 播放线程：异步朗读期间持续收取取消命令，完成事件与代次一起交付。

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};

use super::command::Command;
use super::result::DemoResult;
use super::speaker::Speaker;

const POLL_INTERVAL: Duration = Duration::from_millis(15);
const PLAYBACK_TIMEOUT: Duration = Duration::from_secs(60 * 30);

pub(super) fn run(
    commands: Receiver<Command>,
    results: Sender<(u64, DemoResult)>,
    desired: Arc<AtomicU64>,
) {
    let initialized = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok() };
    if initialized.is_err() {
        while let Ok(command) = commands.recv() {
            match command {
                Command::Speak {
                    generation,
                    request_id,
                    ..
                } if current(&desired, generation) => {
                    send(
                        &results,
                        generation,
                        request_id,
                        Err("Windows 语音示范初始化失败，可继续跟读".to_owned()),
                    );
                }
                Command::Shutdown => break,
                _ => {}
            }
        }
        return;
    }
    run_initialized(commands, results, &desired);
    // 所有 SAPI 接口已在 run_initialized 返回前释放。
    unsafe { CoUninitialize() };
}

fn run_initialized(
    commands: Receiver<Command>,
    results: Sender<(u64, DemoResult)>,
    desired: &AtomicU64,
) {
    let mut speaker: Option<Speaker> = None;
    let mut active: Option<(u64, u64, u32, Instant)> = None;
    loop {
        // 空闲时阻塞等命令，仅在播放中检查完成状态，避免每秒约 67 次空转唤醒。
        let received = if active.is_some() {
            commands.recv_timeout(POLL_INTERVAL)
        } else {
            commands.recv().map_err(|_| RecvTimeoutError::Disconnected)
        };
        let command = match received {
            Ok(command) => Some(command),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => break,
        };
        if active.is_some_and(|(generation, ..)| !current(desired, generation)) {
            stop(&mut speaker);
            active = None;
        }
        match command {
            Some(Command::Shutdown) => break,
            Some(Command::Cancel) => {
                stop(&mut speaker);
                active = None;
            }
            Some(Command::Speak {
                generation,
                request_id,
                text,
                rate,
                language,
            }) if current(desired, generation) => {
                stop(&mut speaker);
                active = None;
                if speaker
                    .as_ref()
                    .is_some_and(|speaker| speaker.language() != language)
                {
                    speaker = None;
                }
                if speaker.is_none() {
                    match Speaker::new(language) {
                        Ok(created) => speaker = Some(created),
                        Err(message) => {
                            if current(desired, generation) {
                                send(&results, generation, request_id, Err(message));
                            }
                            continue;
                        }
                    }
                }
                // 创建 COM 对象可能比取消命令慢，回到播放调用前重查原子代次。
                if !current(desired, generation) {
                    continue;
                }
                match speaker.as_ref().expect("已创建播放器").speak(&text, rate) {
                    Ok(stream) => {
                        active = Some((generation, request_id, stream, Instant::now()));
                    }
                    Err(message) => {
                        stop(&mut speaker);
                        if current(desired, generation) {
                            send(&results, generation, request_id, Err(message));
                        }
                    }
                }
            }
            _ => {}
        }
        let Some((generation, request_id, stream, since)) = active else {
            continue;
        };
        if !current(desired, generation) {
            stop(&mut speaker);
            active = None;
            continue;
        }
        let result = if since.elapsed() >= PLAYBACK_TIMEOUT {
            Err("示范朗读播放超时，已停止播放，可继续跟读".to_owned())
        } else {
            speaker.as_ref().expect("活动播放器").poll(stream)
        };
        match result {
            Ok(false) => {}
            finished => {
                // 错误也先 purge 再通知，避免调用方开麦时仍播放上一段声音。
                stop(&mut speaker);
                active = None;
                if current(desired, generation) {
                    send(&results, generation, request_id, finished.map(|_| ()));
                }
            }
        }
    }
    stop(&mut speaker);
}

fn current(desired: &AtomicU64, generation: u64) -> bool {
    desired.load(Ordering::Acquire) == generation
}

fn stop(speaker: &mut Option<Speaker>) {
    if speaker
        .as_ref()
        .is_some_and(|speaker| speaker.purge().is_err())
    {
        *speaker = None;
    }
}

fn send(
    results: &Sender<(u64, DemoResult)>,
    generation: u64,
    request_id: u64,
    result: Result<(), String>,
) {
    let _ = results.send((generation, DemoResult { request_id, result }));
}
