//! 设置保存请求复用现有本机管道；后台持有系统注册，避免先保存再发现冲突。

use std::{
    fs::{File, OpenOptions},
    io::{self, Read},
    os::windows::io::AsRawHandle,
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

pub(in crate::panel) fn save(shortcut: PracticeShortcut) -> Result<PracticeShortcut, String> {
    let path = super::super::Settings::config_path();
    let config = Config::load(&path)
        .map_err(|error| format!("无法读取当前普通语音按键，原设置保持不变：{error}"))?;
    if config.shortcut.voice_key.conflicts_with_practice(shortcut) {
        return Err(
            "该快捷键与普通语音转文字的按键冲突，原设置保持不变。请换一个跟读快捷键，或先修改普通语音按键。"
                .to_owned(),
        );
    }
    unsafe {
        let _ = WaitNamedPipeW(&HSTRING::from(DEFAULT_PIPE_NAME), 1000);
    }
    let mut stream = OpenOptions::new()
        .read(true)
        .write(true)
        .open(DEFAULT_PIPE_NAME)
        .map_err(|_| {
            "无法连接 Vakyu 语音后台，快捷键未保存。请先启动输入法后台后重试。".to_owned()
        })?;
    unsafe {
        SetNamedPipeHandleState(
            HANDLE(stream.as_raw_handle()),
            Some(&PIPE_NOWAIT),
            None,
            None,
        )
    }
    .map_err(|error| format!("无法建立快捷键确认连接：{error}"))?;
    write_message(
        &mut stream,
        &ClientMessage::SetPracticeShortcut {
            shortcut: shortcut.key_string(),
        },
    )
    .map_err(|error| format!("快捷键请求未能送达，原设置保持不变：{error}"))?;
    let mut stream = DeadlineReader {
        file: stream,
        deadline: Instant::now() + Duration::from_secs(5),
    };
    match read_message::<_, ServerMessage>(&mut stream) {
        Ok(Some(ServerMessage::Update {
            session,
            voice: None,
            ..
        })) if session.0 == 0 => Ok(shortcut),
        Ok(Some(ServerMessage::Update {
            voice: Some(VoiceUpdate::Error { message }),
            ..
        })) => Err(message),
        _ => Err(
            "后台没有确认保存，请确认已安装新版 Vakyu 并重试；界面继续保留原快捷键。".to_owned(),
        ),
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
