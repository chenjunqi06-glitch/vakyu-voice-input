//! 随包自然人声：在独立 Python 进程里用 sherpa-onnx 运行 Piper 英语模型（LibriTTS-R 多说话人），
//! 本机合成并播放，不联网、不保存音频。进程空闲 10 分钟自行退出，下次朗读时重新启动。

mod event;
mod process;

use std::path::{Path, PathBuf};
use std::time::Duration;

use self::event::Event;
use self::process::Process;

/// 自然人声需要的文件；缺了就用系统声音，不影响跟读本身。
const REQUIRED_FILES: &[&str] = &[
    "python/pythonw.exe",
    "python/Lib/site-packages/sherpa_onnx/__init__.py",
    "python/Lib/site-packages/sounddevice.py",
    "app/speak_main.py",
    "app/speech.py",
    "models/speech/en/model.onnx",
    "models/speech/en/tokens.txt",
    "models/speech/en/espeak-ng-data/phontab",
    "models/speech/en/espeak-ng-data/en_dict",
];

const READY_TIMEOUT: Duration = Duration::from_secs(40);

pub(super) struct NaturalVoice {
    root: PathBuf,

    speaker: u16,

    process: Option<Process>,

    /// 每句的编号，同时作为示范线程的播放流号；只增不减，重启进程后继续累加。
    next: u32,
}

impl NaturalVoice {
    pub fn set_speaker(&mut self, speaker: u16) {
        self.speaker = speaker;
    }

    pub fn prewarm(&mut self) -> Result<(), String> {
        if !self.process.as_mut().is_some_and(Process::alive) {
            self.process = None;
            self.process = Some(Process::start(&self.root, READY_TIMEOUT)?);
        }
        Ok(())
    }

    pub fn start(speaker: u16) -> Result<Self, String> {
        let root = crate::voice::neural::runtime_root()
            .filter(|root| available(root))
            .ok_or_else(|| "自然人声组件没有安装完整".to_owned())?;
        let process = Process::start(&root, READY_TIMEOUT)?;
        Ok(Self {
            root,
            speaker,
            process: Some(process),
            next: 0,
        })
    }

    pub fn speak(&mut self, text: &str, rate: i32) -> Result<u32, String> {
        self.next = self.next.wrapping_add(1).max(1);
        let id = self.next;
        let line = serde_json::json!({
            "command": "speak",
            "id": id,
            "text": text,
            "speaker": self.speaker,
            "speed": speed(rate),
        })
        .to_string();
        for attempt in 0..2 {
            self.prewarm()?;
            let process = self.process.as_mut().expect("刚启动朗读进程");
            if process.send(&line).is_ok() {
                return Ok(id);
            }
            tracing::info!(attempt, "自然人声进程已退出，重新启动");
            self.process = None;
        }
        Err("自然人声进程无法启动".to_owned())
    }

    pub fn poll(&mut self, stream: u32) -> Result<bool, String> {
        let outcome = {
            let Some(process) = &self.process else {
                return Err("自然人声进程已退出，可继续跟读".to_owned());
            };
            loop {
                match process.next_event() {
                    Ok(Some(Event::Done(id))) if id == stream => break Ok(true),
                    Ok(Some(Event::Failed(id, message))) if id == stream => break Err(message),
                    Ok(Some(Event::Fatal(message))) => break Err(message),
                    Ok(Some(_)) => {}
                    Ok(None) => break Ok(false),
                    Err(()) => break Err("自然人声进程已退出，可继续跟读".to_owned()),
                }
            }
        };
        if outcome.is_err() && !self.process.as_mut().is_some_and(Process::alive) {
            self.process = None;
        }
        outcome
    }

    /// 停掉正在播放和已排队的句子；之后旧句子不会再回报完成。
    pub fn purge(&mut self) -> Result<(), String> {
        let Some(process) = &mut self.process else {
            return Ok(());
        };
        if process.send(r#"{"command":"stop"}"#).is_err() {
            self.process = None;
            return Ok(());
        }
        while let Ok(Some(_)) = process.next_event() {}
        Ok(())
    }
}

fn available(root: &Path) -> bool {
    REQUIRED_FILES.iter().all(|file| {
        root.join(file)
            .metadata()
            .is_ok_and(|meta| meta.is_file() && meta.len() > 0)
    })
}

/// 把 SAPI 的语速档位（-10..=10，-2 是学习用慢速）换成合成倍速：每 10 档快或慢 3 倍，与 SAPI 一致。
fn speed(rate: i32) -> f32 {
    3f32.powf(rate.clamp(-10, 10) as f32 / 10.0)
}

#[cfg(test)]
mod tests {
    use super::speed;

    #[test]
    fn rate_maps_to_speed_like_sapi() {
        assert!((speed(0) - 1.0).abs() < 1e-6);
        assert!((speed(-2) - 0.8027).abs() < 1e-3);
        assert!((speed(10) - 3.0).abs() < 1e-6);
        assert!((speed(-99) - 1.0 / 3.0).abs() < 1e-6);
    }
}
