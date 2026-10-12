//! 设置保存请求复用现有本机管道；后台持有系统注册，避免先保存再发现冲突。
//! 后台没有回应（没在运行、旧版不认这条请求、一时太忙）时以配置文件为准：
//! 已写进去就算成功，否则直接写配置，由后台热加载或下次启动接上。

use std::{
    fs::{File, OpenOptions},
    io::{self, Read},
    os::windows::io::AsRawHandle,
    path::Path,
    time::{Duration, Instant},
};

use qingjian_platform::{
    Config, PracticeShortcut,
    protocol::{
        ClientMessage, DEFAULT_PIPE_NAME, ServerMessage, VoiceUpdate, read_message, write_message,
    },
};
use windows::Win32::{
    Foundation::HANDLE,
    System::Pipes::{PIPE_NOWAIT, SetNamedPipeHandleState, WaitNamedPipeW},
};
use windows::core::HSTRING;

use super::Saved;

/// 等后台回执的上限。后台自己最多等语音线程 3 秒，这里留出排队余量。
const REPLY_TIMEOUT: Duration = Duration::from_secs(8);

/// 后台没有给出明确结果的原因。
enum NoReply {
    /// 连不上管道：后台没在运行。
    NotRunning,

    /// 连上了但没有回执：超时、连接被关（旧版后台不认这条请求）或回包不对。
    Silent(String),
}

pub(in crate::panel) fn save(shortcut: PracticeShortcut) -> Result<Saved, String> {
    let path = super::super::Settings::config_path();
    let config = Config::load(&path)
        .map_err(|error| format!("无法读取当前普通语音按键，原设置保持不变：{error}"))?;
    if config.shortcut.voice_key.conflicts_with_practice(shortcut) {
        return Err(
            "该快捷键与普通语音转文字的按键冲突，原设置保持不变。请换一个跟读快捷键，或先修改普通语音按键。"
                .to_owned(),
        );
    }
    let no_reply = match request(shortcut) {
        Ok(Ok(())) => {
            return Ok(Saved {
                shortcut,
                confirmed: true,
                running: true,
            });
        }
        // 后台明确拒绝（冲突、语音未就绪等）：原样告诉用户，不绕过它写配置。
        Ok(Err(message)) => return Err(message),
        Err(no_reply) => no_reply,
    };
    let running = !matches!(no_reply, NoReply::NotRunning);
    if let NoReply::Silent(reason) = &no_reply {
        crate::log::warn(format!(
            "跟读快捷键：后台没有回执（{reason}），改为直接写配置"
        ));
    }
    // 后台可能已经保存、只是回执没送到。
    if saved_in_config(&path, shortcut) {
        return Ok(Saved {
            shortcut,
            confirmed: false,
            running,
        });
    }
    Config::set_value(
        &path,
        "shortcut",
        "voice_practice_shortcut",
        shortcut.key_string(),
    )
    .map_err(|error| format!("后台没有回应，直接写入设置也失败，原快捷键保持不变：{error}"))?;
    Ok(Saved {
        shortcut,
        confirmed: false,
        running,
    })
}

fn saved_in_config(path: &Path, shortcut: PracticeShortcut) -> bool {
    Config::load(path).is_ok_and(|config| config.shortcut.voice_practice_shortcut == shortcut)
}

/// 外层 `Err` 是没拿到明确结果；内层是后台给出的结果。
fn request(shortcut: PracticeShortcut) -> Result<Result<(), String>, NoReply> {
    unsafe {
        let _ = WaitNamedPipeW(&HSTRING::from(DEFAULT_PIPE_NAME), 1000);
    }
    let stream = OpenOptions::new()
        .read(true)
        .write(true)
        .open(DEFAULT_PIPE_NAME)
        .map_err(|_| NoReply::NotRunning)?;
    unsafe {
        SetNamedPipeHandleState(
            HANDLE(stream.as_raw_handle()),
            Some(&PIPE_NOWAIT),
            None,
            None,
        )
    }
    .map_err(|error| NoReply::Silent(format!("无法设置管道模式：{error}")))?;
    let mut stream = DeadlineReader {
        file: stream,
        deadline: Instant::now() + REPLY_TIMEOUT,
    };
    write_message(
        &mut stream.file,
        &ClientMessage::SetPracticeShortcut {
            shortcut: shortcut.key_string(),
        },
    )
    .map_err(|error| NoReply::Silent(format!("请求未能送达：{error}")))?;
    match read_message::<_, ServerMessage>(&mut stream) {
        Ok(Some(ServerMessage::Update {
            session,
            voice: None,
            ..
        })) if session.0 == 0 => Ok(Ok(())),
        Ok(Some(ServerMessage::Update {
            voice: Some(VoiceUpdate::Error { message }),
            ..
        })) => Ok(Err(message)),
        Ok(Some(_)) => Err(NoReply::Silent("回包类型不对".to_owned())),
        Ok(None) => Err(NoReply::Silent("后台关闭了连接，可能是旧版后台".to_owned())),
        Err(error) => Err(NoReply::Silent(format!("读取回执失败：{error}"))),
    }
}

struct DeadlineReader {
    file: File,

    deadline: Instant,
}

impl Read for DeadlineReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        loop {
            match self.file.read(buffer) {
                Err(error)
                    if error.raw_os_error() == Some(232)
                        || error.kind() == io::ErrorKind::WouldBlock =>
                {
                    if Instant::now() >= self.deadline {
                        return Err(io::Error::new(
                            io::ErrorKind::TimedOut,
                            "快捷键保存确认超时",
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                result => return result,
            }
        }
    }
}
