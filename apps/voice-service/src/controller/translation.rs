//! 只在原话最终完成时翻译一次：请宿主用系统翻译（macOS Translation 框架，本机运行，不联网、不花钱），
//! 宿主以 `translated` 回报；取消、新一轮或失败后到达的回报按轮次丢弃。

use super::Controller;
use crate::protocol::Event;
use qingjian_core::Language;
use qingjian_learning::PracticeMatchConfig;
use qingjian_practice::{Phase, PracticeSession};
use qingjian_predict::tidy_translation;

impl Controller {
    pub(super) fn begin_practice(&mut self, text: String) {
        let (level, progress) = self.game.begin();
        let shortcut = &self.config.shortcut;
        self.practice = Some(
            PracticeSession::new_for_language(
                text.clone(),
                level,
                progress,
                PracticeMatchConfig {
                    word_similarity: shortcut.voice_practice_similarity,
                    name_similarity: shortcut.voice_practice_name_similarity,
                },
                self.learn_english(),
            )
            .with_send_translation(shortcut.voice_practice_send_translation),
        );
        self.show_practice();
        self.emit(Event::TranslateRequest {
            id: self.round,
            text,
            target: self.target_language().to_owned(),
        });
    }

    fn target(&self) -> Language {
        if self.learn_english() {
            Language::English
        } else {
            Language::Chinese
        }
    }

    fn translating(&self, id: u64) -> bool {
        self.active
            && id == self.round
            && self
                .practice
                .as_ref()
                .is_some_and(|session| session.phase == Phase::Translating)
    }

    /// 宿主的系统翻译完成（或失败）；译文经 Core 的同一套规则校验整理。
    pub(super) fn translated_by_host(&mut self, id: u64, ok: bool, text: String) {
        if !self.translating(id) {
            return;
        }
        if !ok {
            self.fail(text);
            return;
        }
        match tidy_translation(&text, self.target(), false) {
            Ok(text) => self.accept_translation(text),
            Err(error) => self.fail(self.message(error.user_message(), &error.to_string())),
        }
    }

    fn accept_translation(&mut self, text: String) {
        if self.practice.as_mut().expect("已有会话").translated(&text) {
            if self.config.shortcut.voice_practice_demo {
                self.practice.as_mut().expect("已有会话").demonstrating();
                self.speak(text, true);
            } else {
                self.start_practice_capture();
            }
        }
        self.show_practice();
    }
}
