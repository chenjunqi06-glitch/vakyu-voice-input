//! 只在原话最终完成时翻译一次，取消和配置变化撤销旧请求。

use super::Controller;
use qingjian_core::Language;
use qingjian_learning::PracticeMatchConfig;
use qingjian_practice::{Phase, PracticeSession};
use qingjian_predict::VoicePracticeTranslator;

impl Controller {
    pub(super) fn begin_practice(&mut self, text: String) {
        let (level, progress) = self.game.begin();
        let shortcut = &self.config.shortcut;
        self.practice = Some(PracticeSession::new_for_language(
            text.clone(),
            level,
            progress,
            PracticeMatchConfig {
                word_similarity: shortcut.voice_practice_similarity,
                name_similarity: shortcut.voice_practice_name_similarity,
            },
            self.learn_english(),
        ));
        self.show_practice();
        if self.translator.is_none() {
            match VoicePracticeTranslator::new(&self.config.predict) {
                Ok(translator) => self.translator = Some(translator),
                Err(error) => {
                    self.fail(self.message(error.user_message(), &error.to_string()));
                    return;
                }
            }
        }
        let target = if self.learn_english() {
            Language::English
        } else {
            Language::Chinese
        };
        if let Err(error) = self
            .translator
            .as_mut()
            .expect("翻译器已创建")
            .request_to(self.round, &text, target)
        {
            self.fail(self.message(error.user_message(), &error.to_string()));
        }
    }

    pub(super) fn translation_tick(&mut self) {
        let Some(result) = self
            .translator
            .as_mut()
            .and_then(VoicePracticeTranslator::try_recv)
        else {
            return;
        };
        if !self.active
            || result.request_id != self.round
            || !self
                .practice
                .as_ref()
                .is_some_and(|session| session.phase == Phase::Translating)
        {
            return;
        }
        match result.result {
            Ok(text) => {
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
            Err(error) => self.fail(self.message(error.user_message(), &error.to_string())),
        }
    }
}
