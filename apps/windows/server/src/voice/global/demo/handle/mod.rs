//! 非阻塞的示范入口；原子代次先于命令队列生效，过期的朗读不能重新开始。

use qingjian_core::Language;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};

use super::command::Command;
use super::result::DemoResult;
use super::worker;

pub(in crate::voice::global) struct EnglishDemo {
    commands: Sender<Command>,

    results: Receiver<(u64, DemoResult)>,

    desired: Arc<AtomicU64>,

    generation: u64,

    active: Option<(u64, u64)>,

    /// 英语示范的自然人声音色；`None` 用系统声音。由调用方按当前配置在朗读前设置。
    natural: Option<u16>,
}

impl EnglishDemo {
    /// 只创建线程，不朗读，不打开麦克风；语言和音频设备错误由本轮结果报告。
    pub fn new() -> Result<Self, String> {
        let (commands, command_rx) = mpsc::channel();
        let (result_tx, results) = mpsc::channel();
        let desired = Arc::new(AtomicU64::new(0));
        let worker_desired = Arc::clone(&desired);
        std::thread::Builder::new()
            .name("miracle-english-demo".to_owned())
            .spawn(move || worker::run(command_rx, result_tx, worker_desired))
            .map_err(|_| "示范朗读线程无法启动，可继续跟读".to_owned())?;
        Ok(Self {
            commands,
            results,
            desired,
            generation: 0,
            active: None,
            natural: None,
        })
    }

    pub fn set_natural_voice(&mut self, natural: Option<u16>) {
        self.natural = natural;
    }

    /// 速率由学习等级决定；-2 为慢速，0 为系统正常速度。
    pub fn speak(&mut self, request_id: u64, text: &str, rate: i32) -> Result<(), String> {
        self.speak_inner(request_id, text, rate, Language::English)
    }

    pub fn speak_in_language(
        &mut self,
        request_id: u64,
        text: &str,
        rate: i32,
        language: Language,
    ) -> Result<(), String> {
        if language == Language::English {
            self.speak(request_id, text, rate)
        } else {
            self.speak_inner(request_id, text, rate, language)
        }
    }

    fn speak_inner(
        &mut self,
        request_id: u64,
        text: &str,
        rate: i32,
        language: Language,
    ) -> Result<(), String> {
        self.cancel();
        if !matches!(language, Language::Chinese | Language::English) {
            return Err("示范朗读目前支持中文和英语".to_owned());
        }
        let text = text.trim();
        if text.is_empty() {
            return Err("没有可朗读的译句".to_owned());
        }
        if text.contains('\0') {
            return Err("译句包含无效字符，可继续跟读".to_owned());
        }
        self.active = Some((self.generation, request_id));
        if self
            .commands
            .send(Command::Speak {
                generation: self.generation,
                request_id,
                text: text.to_owned(),
                rate: rate.clamp(-10, 10),
                language,
                natural: self.natural,
            })
            .is_err()
        {
            self.active = None;
            return Err("示范朗读服务已退出，可继续跟读".to_owned());
        }
        Ok(())
    }

    /// 同一轮最多交付一次；取消后的完成事件不能启动下一轮麦克风。
    pub fn try_recv(&mut self) -> Option<DemoResult> {
        let (expected, request_id) = self.active?;
        loop {
            match self.results.try_recv() {
                Ok((generation, result)) if generation == expected => {
                    self.active = None;
                    return Some(result);
                }
                Ok(_) => {}
                Err(TryRecvError::Empty) => return None,
                Err(TryRecvError::Disconnected) => {
                    self.active = None;
                    return Some(DemoResult {
                        request_id,
                        result: Err("示范朗读服务已退出，可继续跟读".to_owned()),
                    });
                }
            }
        }
    }

    /// 先撤销旧轮次，再唤醒播放线程执行 purge；调用方取消时也必须关闭跟读麦克风。
    pub fn cancel(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.desired.store(self.generation, Ordering::Release);
        self.active = None;
        let _ = self.commands.send(Command::Cancel);
        while self.results.try_recv().is_ok() {}
    }
}

impl Drop for EnglishDemo {
    fn drop(&mut self) {
        self.cancel();
        let _ = self.commands.send(Command::Shutdown);
    }
}

#[cfg(test)]
mod tests;
