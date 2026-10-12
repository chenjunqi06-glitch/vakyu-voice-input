//! 词卡查询与显式放行；播放示范时暂停收音但不推进任何词。

use super::Controller;
use crate::protocol::Event;
use qingjian_practice::{Phase, PracticeAction, lookup};
use std::time::{Duration, Instant};

impl Controller {
    pub(super) fn action(&mut self, action: PracticeAction, index: Option<usize>) {
        if self.pending_commit.is_some() {
            return;
        }
        if self.requires_new_round
            && !matches!(action, PracticeAction::Lookup | PracticeAction::CloseCard)
        {
            self.emit(Event::Error {
                message: self.message(
                    "设置已改变，请按 Esc 取消后重新录音",
                    "Settings changed; press Esc and start a new recording",
                ),
            });
            return;
        }
        if action == PracticeAction::Retry && self.practice.is_none() {
            if let Some((_, text)) = &self.delivery {
                self.commit(text.clone());
            }
            return;
        }
        if action == PracticeAction::Retry
            && self.practice.as_ref().is_some_and(|session| {
                session.phase == Phase::Failed && session.view().words.is_empty()
            })
        {
            let original = self.practice.as_ref().expect("已有会话").view().original;
            self.begin_practice(original);
            return;
        }
        let Some(session) = &mut self.practice else {
            return;
        };
        match action {
            PracticeAction::Lookup => {
                let Some(index) = index else {
                    return;
                };
                let view = session.view();
                let word = if view.learn_english {
                    view.words.get(index).cloned()
                } else {
                    qingjian_translate::chinese_practice_word(&view.words, index)
                        .or_else(|| view.words.get(index).cloned())
                };
                if let Some(word) = word {
                    session
                        .show_word_card(lookup::word_card_for_language(&word, view.learn_english));
                }
                self.show_practice();
                return;
            }
            PracticeAction::CloseCard => {
                session.close_word_card();
                self.show_practice();
                return;
            }
            PracticeAction::SpeakWord => {
                let Some(card) = session.view().word_card else {
                    return;
                };
                if !matches!(session.phase, Phase::Reading | Phase::Failed) {
                    return;
                }
                if session.phase == Phase::Reading {
                    session.pause_for_pronunciation();
                }
                self.speak(card.word, false);
                return;
            }
            _ => {}
        }
        let accepted = match action {
            PracticeAction::Retry => session.retry_current(),
            PracticeAction::Skip => session.skip_current(),
            PracticeAction::Accept => session.accept_current(),
            _ => false,
        };
        if !accepted {
            return;
        }
        self.cancel_capture();
        self.speech = None;
        self.speech_deadline = None;
        self.resume_at = None;
        self.emit(Event::CancelSpeech);
        if let Some(original) = self
            .practice
            .as_ref()
            .and_then(|session| session.completed_original())
            .map(str::to_owned)
        {
            self.commit(original);
        } else {
            self.start_practice_capture();
        }
        self.show_practice();
    }

    pub(super) fn speak(&mut self, text: String, demonstration: bool) {
        self.cancel_capture();
        self.emit(Event::CancelSpeech);
        let id = self.next_id();
        let language = lookup::pronunciation_language(&text, self.learn_english());
        let language = if language == qingjian_core::Language::English {
            "en"
        } else {
            "zh"
        }
        .to_owned();
        let rate = if demonstration {
            self.practice
                .as_ref()
                .map_or(0, |session| session.demo_rate())
        } else {
            0
        };
        self.speech = Some((id, demonstration));
        self.speech_deadline =
            Some(Instant::now() + Duration::from_secs(30 + text.chars().count() as u64 * 2));
        self.resume_at = None;
        self.emit(Event::Speak {
            id,
            text,
            language,
            rate,
        });
        self.show_practice();
    }

    pub(super) fn speech_finished(&mut self, id: u64, ok: bool) {
        let Some((pending, demonstration)) = self.speech else {
            return;
        };
        if pending != id {
            return;
        }
        self.speech = None;
        self.speech_deadline = None;
        let hint = self.message(
            if ok {
                "发音播放完成"
            } else {
                "本机发音暂不可用"
            },
            if ok {
                "Pronunciation finished"
            } else {
                "Local pronunciation is unavailable"
            },
        );
        if let Some(session) = &mut self.practice {
            if demonstration {
                session.demo_finished(!ok);
            } else {
                session.set_word_card_hint(Some(hint));
            }
            if session.phase == Phase::Preparing {
                self.resume_at = Some(Instant::now() + Duration::from_millis(400));
            }
        }
        self.show_practice();
    }
}
