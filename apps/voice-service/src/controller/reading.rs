//! 划词翻译：宿主从系统「服务」菜单交来选中文字，译文以 `translation` 事件返回，不碰跟读轮次和输入框。

use qingjian_core::Language;
use qingjian_predict::VoicePracticeTranslator;

use super::Controller;
use crate::protocol::Event;

impl Controller {
    pub(super) fn translate_selection(&mut self, id: u64, text: &str) {
        if self.reader.is_none() {
            match VoicePracticeTranslator::new_reading(&self.config.predict) {
                Ok(reader) => self.reader = Some(reader),
                Err(error) => {
                    self.reading_failed(
                        id,
                        error.user_message(),
                        qingjian_predict::reading_target(text),
                    );
                    return;
                }
            }
        }
        match self
            .reader
            .as_mut()
            .expect("已创建划词翻译器")
            .request_reading(id, text)
        {
            Ok(target) => self.reading = Some((id, target)),
            Err(error) => {
                self.reading = None;
                self.reading_failed(
                    id,
                    error.user_message(),
                    qingjian_predict::reading_target(text),
                );
            }
        }
    }

    pub(super) fn reading_tick(&mut self) {
        let Some((id, target)) = self.reading else {
            return;
        };
        let Some(result) = self
            .reader
            .as_mut()
            .and_then(VoicePracticeTranslator::try_recv)
        else {
            return;
        };
        if result.request_id != id {
            return;
        }
        self.reading = None;
        match result.result {
            Ok(text) => self.emit(Event::Translation {
                id,
                ok: true,
                text,
                target: language_code(target).to_owned(),
            }),
            Err(error) => self.reading_failed(id, error.user_message(), target),
        }
    }

    /// 云服务设置变了：丢掉旧凭据的实例，正在等的请求按失败回给宿主。
    pub(super) fn drop_reader(&mut self) {
        if let Some(reader) = &mut self.reader {
            reader.cancel();
        }
        self.reader = None;
        if let Some((id, target)) = self.reading.take() {
            self.reading_failed(id, "翻译设置已更新，请重新翻译。", target);
        }
    }

    fn reading_failed(&mut self, id: u64, message: &str, target: Language) {
        self.emit(Event::Translation {
            id,
            ok: false,
            text: message.to_owned(),
            target: language_code(target).to_owned(),
        });
    }
}

fn language_code(language: Language) -> &'static str {
    if language == Language::English {
        "en"
    } else {
        "zh"
    }
}
