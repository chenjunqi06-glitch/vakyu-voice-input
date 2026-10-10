//! 单线程会话控制器；网络和识别只通过异步消息交付，界面永不等待它们。

mod actions;
mod capture;
mod settings;
mod translation;

#[cfg(test)]
mod tests;

use crate::{
    game::PracticeGame,
    protocol::{AsrEvent, Command, Effect, Event, Mode},
};
use qingjian_platform::Config;
use qingjian_practice::{Phase, PracticeSession};
use qingjian_predict::VoicePracticeTranslator;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

pub struct Controller {
    config: Config,

    config_path: PathBuf,

    data: PathBuf,

    game: PracticeGame,

    practice: Option<PracticeSession>,

    translator: Option<VoicePracticeTranslator>,

    round: u64,

    generation: u64,

    active: bool,

    round_mode: Mode,

    capturing: bool,

    ready: bool,

    stopping_at: Option<Instant>,

    capture_started_at: Option<Instant>,

    speech: Option<(u64, bool)>,

    speech_deadline: Option<Instant>,

    resume_at: Option<Instant>,

    sequence: u64,

    pending_commit: Option<(u64, String, Instant)>,

    delivery: Option<(u64, String)>,

    requires_new_round: bool,

    dictation: String,

    effects: Vec<Effect>,
}

impl Controller {
    pub fn new(config: Config, config_path: PathBuf, data: PathBuf) -> Self {
        let game = PracticeGame::load(&data, config.shortcut.voice_practice_learn_english);
        let mut controller = Self {
            config,
            config_path,
            data,
            game,
            practice: None,
            translator: None,
            round: 0,
            generation: 0,
            active: false,
            round_mode: Mode::Dictation,
            capturing: false,
            ready: false,
            stopping_at: None,
            capture_started_at: None,
            speech: None,
            speech_deadline: None,
            resume_at: None,
            sequence: 0,
            pending_commit: None,
            delivery: None,
            requires_new_round: false,
            dictation: String::new(),
            effects: Vec::new(),
        };
        controller.emit_config();
        controller
    }

    pub fn drain(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.effects)
    }

    pub fn poll_interval(&self) -> Duration {
        if self.ready && !self.active {
            Duration::from_secs(1)
        } else {
            Duration::from_millis(20)
        }
    }

    fn emit(&mut self, event: Event) {
        self.effects.push(Effect::Native(event));
    }

    fn asr(&mut self, value: serde_json::Value) {
        self.effects.push(Effect::Asr(value));
    }

    fn learn_english(&self) -> bool {
        self.config.shortcut.voice_practice_learn_english
    }

    fn target_language(&self) -> &'static str {
        if self.learn_english() { "en" } else { "zh" }
    }

    fn source_language(&self) -> &'static str {
        if self.round_mode == Mode::Practice && !self.learn_english() {
            "en"
        } else {
            "zh"
        }
    }

    fn next_id(&mut self) -> u64 {
        self.sequence = self.sequence.wrapping_add(1);
        self.sequence
    }

    pub fn command(&mut self, command: Command) -> bool {
        match command {
            Command::Press { mode } => self.press(mode),
            Command::Release => self.release(),
            Command::Cancel => self.cancel(),
            Command::Audio {
                generation,
                sample_rate,
                pcm,
            } => self.audio(generation, sample_rate, pcm),
            Command::CaptureFailed {
                generation,
                message,
            } => {
                if self.active && generation == self.generation {
                    self.fail(message);
                }
            }
            Command::SpeechFinished { id, ok } => self.speech_finished(id, ok),
            Command::Action { action, index } => self.action(action, index),
            Command::CommitResult { id, ok, message } => self.commit_result(id, ok, message),
            Command::Set { key, value } => self.set(&key, value),
            Command::Reload => self.reload(),
            Command::Shutdown => {
                self.cancel();
                return false;
            }
        }
        true
    }

    pub fn cancel(&mut self) {
        self.cancel_capture();
        self.emit(Event::CancelSpeech);
        if let Some(translator) = &mut self.translator {
            translator.cancel();
        }
        self.game.cancel();
        self.practice = None;
        self.pending_commit = None;
        self.delivery = None;
        self.requires_new_round = false;
        self.speech = None;
        self.speech_deadline = None;
        self.resume_at = None;
        self.active = false;
        self.dictation.clear();
        self.round = self.round.wrapping_add(1);
        self.emit(Event::Hide);
    }

    fn show_practice(&mut self) {
        if let Some(session) = &self.practice {
            let mut view = session.view();
            if session.phase == Phase::Failed && view.words.is_empty() && !self.requires_new_round {
                view.actions_enabled = true;
            }
            if self.requires_new_round {
                view.actions_enabled = false;
            }
            self.emit(Event::Practice {
                view: Box::new(view),
            });
        }
    }

    pub fn fail(&mut self, message: String) {
        self.cancel_capture();
        self.speech = None;
        self.speech_deadline = None;
        self.resume_at = None;
        self.emit(Event::CancelSpeech);
        if let Some(translator) = &mut self.translator {
            translator.cancel();
        }
        if let Some(session) = &mut self.practice {
            session.fail(message.clone());
            self.show_practice();
        } else if self.delivery.is_none() {
            self.active = false;
        }
        self.emit(Event::Error { message });
    }

    fn message(&self, chinese: &str, english: &str) -> String {
        if self.learn_english() {
            chinese
        } else {
            english
        }
        .to_owned()
    }

    pub fn tick(&mut self, now: Instant) {
        if self.speech_deadline.is_some_and(|deadline| now >= deadline) {
            self.fail(self.message(
                "本机朗读未响应，请重试",
                "Local speech playback did not respond; please retry",
            ));
        }
        if self
            .capture_started_at
            .is_some_and(|at| now.duration_since(at) > Duration::from_secs(20))
        {
            self.fail(self.message(
                "语音模型未响应，请重试",
                "Speech recognition did not respond; please retry",
            ));
        }
        if self
            .stopping_at
            .is_some_and(|at| now.duration_since(at) > Duration::from_secs(25))
        {
            self.fail(self.message(
                "语音收尾超时，原话尚未填入",
                "Speech finalization timed out; no text was inserted",
            ));
        }
        if self
            .practice
            .as_ref()
            .is_some_and(|session| session.timed_out(now))
        {
            self.fail(self.message(
                "本次处理超时，原话尚未填入，请重试",
                "This step timed out; no text was inserted. Please retry",
            ));
        }
        // 不自动重发缺少确认的填入请求，避免宿主已填入但确认丢失后重复正文。
        if self
            .pending_commit
            .as_ref()
            .is_some_and(|(_, _, at)| now.duration_since(*at) > Duration::from_secs(15))
        {
            self.pending_commit = None;
            self.fail(self.message(
                "未收到输入框确认，请检查是否已经填入后再重试",
                "No insertion confirmation; check the field before retrying",
            ));
        }
        self.translation_tick();
        if self.resume_at.is_some_and(|at| now >= at) {
            self.resume_at = None;
            self.start_practice_capture();
        }
        if self.speech.is_none()
            && self
                .practice
                .as_ref()
                .is_some_and(|session| session.should_check(now))
        {
            self.practice.as_mut().expect("已有会话").checking();
            self.stop_capture();
            self.show_practice();
        }
    }

    fn commit(&mut self, text: String) {
        if !self.active
            || self.requires_new_round
            || self.pending_commit.is_some()
            || text.trim().is_empty()
        {
            return;
        }
        if self
            .practice
            .as_ref()
            .is_some_and(|session| session.phase != Phase::Complete)
        {
            return;
        }
        self.cancel_capture();
        let id = if let Some((id, previous)) = &self.delivery {
            if previous != &text {
                return;
            }
            *id
        } else {
            let id = self.next_id();
            self.delivery = Some((id, text.clone()));
            id
        };
        self.pending_commit = Some((id, text.clone(), Instant::now()));
        self.emit(Event::Commit { id, text });
    }

    fn commit_result(&mut self, id: u64, ok: bool, message: Option<String>) {
        if !self
            .pending_commit
            .as_ref()
            .is_some_and(|(pending, _, _)| *pending == id)
        {
            return;
        }
        self.pending_commit = None;
        if ok {
            if let Some(session) = &mut self.practice {
                self.game.complete(session.progress());
                session.take_original();
            }
            self.cancel();
        } else {
            self.fail(message.unwrap_or_else(|| {
                self.message(
                    "输入框暂不可用，请恢复后重试",
                    "The input field is unavailable; restore it and retry",
                )
            }));
        }
    }

    pub fn backend(&mut self, event: AsrEvent) {
        self.handle_backend(event);
    }
}
