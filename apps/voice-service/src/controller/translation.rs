//! 只在原话最终完成时翻译一次，取消和配置变化撤销旧请求。
//! `[predict] translation_engine = "local"`（默认）时请宿主用系统翻译（macOS Translation 框架，本机、免费），
//! 宿主报告系统翻译不可用时，填了密钥才退回云端接口。

use super::Controller;
use crate::protocol::Event;
use qingjian_core::Language;
use qingjian_learning::PracticeMatchConfig;
use qingjian_practice::{Phase, PracticeSession};
use qingjian_predict::{TranslationEngine, VoicePracticeTranslator, tidy_translation};

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
        if self.config.predict.translation_engine == TranslationEngine::Local {
            self.emit(Event::TranslateRequest {
                id: self.round,
                text,
                target: self.target_language().to_owned(),
            });
            return;
        }
        self.request_cloud(&text);
    }

    fn request_cloud(&mut self, text: &str) {
        if self.translator.is_none() {
            match VoicePracticeTranslator::new(&self.config.predict) {
                Ok(translator) => self.translator = Some(translator),
                Err(error) => {
                    self.fail(self.message(error.user_message(), &error.to_string()));
                    return;
                }
            }
        }
        if let Err(error) = self.translator.as_mut().expect("翻译器已创建").request_to(
            self.round,
            text,
            self.target(),
        ) {
            self.fail(self.message(error.user_message(), &error.to_string()));
        }
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

    /// 宿主用系统翻译完成（或失败）。`fallback` 表示本机没有系统翻译，可以改用云端。
    pub(super) fn translated_by_host(&mut self, id: u64, ok: bool, text: String, fallback: bool) {
        if !self.translating(id) {
            return;
        }
        if ok {
            match tidy_translation(&text, self.target(), false) {
                Ok(text) => self.accept_translation(text),
                Err(error) => self.fail(self.message(error.user_message(), &error.to_string())),
            }
            return;
        }
        if fallback && VoicePracticeTranslator::new(&self.config.predict).is_ok() {
            let original = self
                .practice
                .as_ref()
                .map(|session| session.original().to_owned())
                .unwrap_or_default();
            self.request_cloud(&original);
            return;
        }
        self.fail(text);
    }

    pub(super) fn translation_tick(&mut self) {
        let Some(result) = self
            .translator
            .as_mut()
            .and_then(VoicePracticeTranslator::try_recv)
        else {
            return;
        };
        if !self.translating(result.request_id) {
            return;
        }
        match result.result {
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
