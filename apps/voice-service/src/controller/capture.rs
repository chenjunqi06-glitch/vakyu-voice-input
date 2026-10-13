//! 麦克风代次与流式识别路由；不以录音总时长终止长句。

use super::Controller;
use crate::protocol::{AsrEvent, Event, Mode};
use qingjian_practice::Phase;
use serde_json::json;
use std::time::Instant;

impl Controller {
    fn capture(&mut self, practice_text: Option<String>) {
        self.generation = self.generation.wrapping_add(1);
        self.capturing = true;
        self.stopping_at = None;
        self.capture_started_at = Some(Instant::now());
        let language = if practice_text.is_some() {
            self.target_language()
        } else {
            self.source_language()
        }
        .to_owned();
        self.asr(json!({"command":"start","generation":self.generation,"language":language,"practice_text":practice_text}));
        self.emit(Event::Capture {
            active: true,
            generation: self.generation,
            language,
            practice_text,
        });
    }

    pub(super) fn press(&mut self, mode: Option<Mode>) {
        if self.active || self.requires_new_round || !self.config.shortcut.voice_input {
            return;
        }
        let mode = mode.unwrap_or(if self.config.shortcut.voice_english_practice {
            Mode::Practice
        } else {
            Mode::Dictation
        });
        if mode == Mode::Practice && !self.config.shortcut.voice_english_practice {
            self.emit(Event::Error {
                message: self.message(
                    "跟读练习已关闭，请先在设置中开启",
                    "Reading practice is disabled; enable it in settings first",
                ),
            });
            return;
        }
        if !self.ready {
            self.emit(Event::Error {
                message: self.message(
                    "语音模型正在准备，请稍后重试",
                    "Speech model is warming up; please retry shortly",
                ),
            });
            return;
        }
        self.active = true;
        self.round_mode = mode;
        self.round = self.round.wrapping_add(1);
        self.dictation.clear();
        self.capture(None);
        self.emit(Event::Dictation {
            text: String::new(),
            recording: true,
            level: 0.0,
        });
    }

    pub(super) fn release(&mut self) {
        if self.active && self.practice.is_none() && self.capturing {
            self.stop_capture();
            self.emit(Event::Dictation {
                text: self.dictation.clone(),
                recording: false,
                level: 0.0,
            });
        }
    }

    pub(super) fn stop_capture(&mut self) {
        if !self.capturing {
            return;
        }
        self.capturing = false;
        self.capture_started_at = None;
        self.emit(Event::Capture {
            active: false,
            generation: self.generation,
            language: self.target_language().to_owned(),
            practice_text: None,
        });
        self.asr(json!({"command":"stop","generation":self.generation}));
        self.stopping_at = Some(Instant::now());
    }

    pub(super) fn cancel_capture(&mut self) {
        self.capturing = false;
        self.capture_started_at = None;
        self.stopping_at = None;
        self.emit(Event::Capture {
            active: false,
            generation: self.generation,
            language: self.target_language().to_owned(),
            practice_text: None,
        });
        self.asr(json!({"command":"cancel","generation":self.generation}));
        self.generation = self.generation.wrapping_add(1);
    }

    pub(super) fn audio(&mut self, generation: u64, sample_rate: u32, pcm: String) {
        if !self.active || !self.capturing || generation != self.generation {
            return;
        }
        if pcm.len() > 262_144 || !(8_000..=96_000).contains(&sample_rate) {
            self.fail(self.message(
                "麦克风音频格式无效，请重试",
                "Invalid microphone audio format; please retry",
            ));
            return;
        }
        self.asr(
            json!({"command":"audio","generation":generation,"sample_rate":sample_rate,"pcm":pcm}),
        );
    }

    pub(super) fn start_practice_capture(&mut self) {
        let Some(session) = &self.practice else {
            return;
        };
        if session.phase != Phase::Preparing || !self.active || self.pending_commit.is_some() {
            return;
        }
        let hint = session.recognition_hint();
        self.capture(Some(hint));
        self.show_practice();
    }

    pub(super) fn handle_backend(&mut self, event: AsrEvent) {
        let (generation, is_final) = match &event {
            AsrEvent::Ready => {
                self.ready = true;
                self.emit(Event::Ready);
                return;
            }
            AsrEvent::Error {
                generation: None,
                message,
            } => {
                self.ready = false;
                self.fail(message.clone());
                return;
            }
            AsrEvent::Error {
                generation: Some(generation),
                ..
            }
            | AsrEvent::Started { generation }
            | AsrEvent::Interim { generation, .. }
            | AsrEvent::Level { generation, .. } => (*generation, false),
            AsrEvent::Final { generation, .. } => (*generation, true),
        };
        if !self.active
            || generation != self.generation
            || (!self.capturing && !is_final && self.stopping_at.is_none())
        {
            return;
        }
        match event {
            AsrEvent::Started { .. } => {
                self.capture_started_at = None;
                if let Some(session) = &mut self.practice {
                    session.started();
                    self.show_practice();
                }
            }
            AsrEvent::Interim { text, .. } => {
                self.capture_started_at = None;
                if let Some(session) = &mut self.practice {
                    if session.interim(&text, Instant::now()) {
                        self.show_practice();
                    }
                } else {
                    self.dictation = text.clone();
                    self.emit(Event::Dictation {
                        text,
                        recording: self.capturing,
                        level: 0.0,
                    });
                }
            }
            AsrEvent::Final { text, .. } => {
                // 识别器不能自行把仍按住的长口述当作整轮完成。
                if self.stopping_at.take().is_none() {
                    self.fail(self.message(
                        "录音意外结束，原话尚未填入，请重试",
                        "Recording stopped unexpectedly; no text was inserted",
                    ));
                    return;
                }
                self.capture_started_at = None;
                if let Some(session) = &mut self.practice {
                    if session.final_result(&text) {
                        let output = session.completed_output().unwrap_or_default().to_owned();
                        self.show_practice();
                        self.commit(output);
                    } else if session.phase == Phase::Preparing {
                        self.start_practice_capture();
                    } else {
                        self.show_practice();
                    }
                } else if text.trim().is_empty() {
                    self.cancel();
                } else if self.round_mode == Mode::Practice {
                    self.begin_practice(text);
                } else {
                    self.dictation = text.clone();
                    self.emit(Event::Dictation {
                        text: text.clone(),
                        recording: false,
                        level: 0.0,
                    });
                    self.commit(text);
                }
            }
            AsrEvent::Level { level, .. } => {
                if self.practice.is_none() {
                    self.emit(Event::Dictation {
                        text: self.dictation.clone(),
                        recording: self.capturing,
                        level: if level.is_finite() {
                            level.clamp(0.0, 1.0)
                        } else {
                            0.0
                        },
                    });
                }
            }
            AsrEvent::Error { message, .. } => self.fail(message),
            AsrEvent::Ready => {}
        }
    }
}
