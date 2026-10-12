//! 中文转写后的异步翻译与跟读；任何失败、取消或焦点变化都不能绕过原文门禁。

use std::time::Instant;

use qingjian_platform::protocol::VoiceUpdate;
use qingjian_predict::VoicePracticeTranslator;

use crate::dispatch::PracticeAction;
use crate::voice::global::practice::{Phase, PracticeSession};

use super::{Controller, empty_rect};

impl Controller {
    pub(super) fn practice_action(&mut self, action: PracticeAction) {
        if action == PracticeAction::Cancel {
            self.cancel();
            return;
        }
        if let PracticeAction::LookupWord(index) = action {
            let word = self.practice.as_ref().and_then(|session| {
                let view = session.view();
                if view.learn_english {
                    view.words.get(index).cloned()
                } else {
                    qingjian_translate::chinese_practice_word(&view.words, index)
                        .or_else(|| view.words.get(index).cloned())
                }
            });
            if let Some(word) = word {
                let card = crate::voice::global::practice::lookup::word_card_for_language(
                    &word,
                    self.config.learn_english,
                );
                self.practice
                    .as_mut()
                    .expect("已取得当前跟读轮次")
                    .show_word_card(card);
                self.show_practice();
            }
            return;
        }
        if action == PracticeAction::CloseWordCard {
            if let Some(session) = &mut self.practice {
                session.close_word_card();
                self.show_practice();
            }
            return;
        }
        if action == PracticeAction::SpeakWord {
            self.speak_word();
            return;
        }
        if self.practice.is_none() {
            return;
        }
        if !self.practice_target_current(true) {
            self.practice_error(
                "原输入框或输入桌面已变化，不能继续跟读操作；请按 Esc 取消".to_owned(),
            );
            return;
        }
        let accepted = {
            let session = self.practice.as_mut().expect("已检查跟读轮次");
            match action {
                PracticeAction::Retry => session.retry_current(),
                PracticeAction::SkipCurrent => session.skip_current(),
                PracticeAction::AcceptCurrent => session.accept_current(),
                PracticeAction::LookupWord(_)
                | PracticeAction::SpeakWord
                | PracticeAction::CloseWordCard
                | PracticeAction::Cancel => false,
            }
        };
        if !accepted {
            return;
        }
        if let Some(demo) = &mut self.demo {
            demo.cancel();
        }
        self.pronunciation_request = None;
        self.demo_ready_at = None;
        self.voice.cancel();
        if self
            .practice
            .as_ref()
            .is_some_and(|session| session.phase == Phase::Complete)
        {
            let original = self
                .practice
                .as_ref()
                .and_then(PracticeSession::completed_original)
                .map(str::to_owned)
                .expect("用户显式放行了句尾");
            self.finish(original);
        } else {
            self.start_practice_recording();
        }
    }

    pub(super) fn dictation_finished(&mut self, text: String) {
        tracing::info!(
            sequence = self.active.as_ref().map(|active| active.sequence),
            characters = text.chars().count(),
            english_practice = self.config.english_practice,
            empty = text.trim().is_empty(),
            "原话口述结束，检查跟读开关"
        );
        if text.trim().is_empty() || !self.config.english_practice {
            self.finish(text);
            return;
        }
        if !self.practice_target_current(false) {
            self.cancel();
            return;
        }
        let Some(active) = &self.active else {
            return;
        };
        let sequence = active.sequence;
        self.voice.cancel();
        let (level, level_progress) = self.game.begin();
        self.practice = Some(PracticeSession::new_for_language(
            text.clone(),
            level,
            level_progress,
            self.config.practice_match,
            self.config.learn_english,
        ));
        self.hook.set_practice_active(true);
        tracing::info!(sequence, "跟读练习开始准备翻译");
        self.sink.set_voice_held(true);
        self.show_practice();
        if crate::voice::neural::runtime_root().is_none() {
            self.practice_error("跟读练习需要安装支持中英识别的语音模型；请按 Esc 取消".to_owned());
            return;
        }
        if self.translator.is_none() {
            match VoicePracticeTranslator::new(&self.config.translation) {
                Ok(translator) => self.translator = Some(translator),
                Err(error) => {
                    tracing::warn!(sequence, ?error, "跟读练习翻译器初始化失败");
                    self.practice_error(error.user_message().to_owned());
                    return;
                }
            }
        }
        if !self.practice_target_current(false) {
            self.practice_error(
                "原输入框已变化，原话尚未填入；恢复输入框后可重试，或按 Esc 取消".to_owned(),
            );
            return;
        }
        if let Err(error) = self.translator.as_mut().expect("已创建翻译器").request_to(
            sequence,
            &text,
            self.config.target_language(),
        ) {
            tracing::warn!(sequence, ?error, "跟读练习翻译请求提交失败");
            self.practice_error(error.user_message().to_owned());
        } else {
            tracing::info!(sequence, "跟读练习翻译请求已提交");
        }
        self.sink.set_voice_held(true);
        self.show_practice();
    }

    pub(super) fn practice_tick(&mut self) {
        if self
            .practice
            .as_ref()
            .is_some_and(|session| session.timed_out(Instant::now()))
        {
            self.practice_error("本次处理超时，原话尚未填入；请按 Esc 取消后重试".to_owned());
            return;
        }
        if let Some(result) = self
            .translator
            .as_mut()
            .and_then(VoicePracticeTranslator::try_recv)
        {
            let current = self
                .active
                .as_ref()
                .is_some_and(|active| active.sequence == result.request_id)
                && self
                    .practice
                    .as_ref()
                    .is_some_and(|session| session.phase == Phase::Translating);
            if current {
                match result.result {
                    Ok(english) => {
                        tracing::info!(
                            sequence = result.request_id,
                            words = english.split_whitespace().count(),
                            "跟读练习译文已返回"
                        );
                        if self
                            .practice
                            .as_mut()
                            .expect("已检查跟读轮次")
                            .translated(&english)
                        {
                            self.start_demonstration(&english);
                        } else {
                            tracing::warn!(
                                sequence = result.request_id,
                                "跟读练习译文无法用于逐词匹配"
                            );
                            self.show_practice();
                        }
                    }
                    Err(error) => {
                        tracing::warn!(sequence = result.request_id, ?error, "跟读练习翻译失败");
                        self.practice_error(error.user_message().to_owned());
                    }
                }
            }
        }
        self.demonstration_tick();
        if let Some(request) = self.pronunciation_request {
            if let Some(result) = self
                .demo
                .as_mut()
                .and_then(crate::voice::global::demo::EnglishDemo::try_recv)
                && result.request_id == request
            {
                self.demo_ready_at = Some((
                    Instant::now() + std::time::Duration::from_millis(400),
                    false,
                ));
                if let Some(session) = &mut self.practice {
                    session.set_word_card_hint(Some(match result.result {
                        Ok(()) => "发音播放完成".to_owned(),
                        Err(error) => error,
                    }));
                }
                self.show_practice();
            }
            if self
                .demo_ready_at
                .is_some_and(|(ready, _)| Instant::now() >= ready)
            {
                self.demo_ready_at = None;
                self.pronunciation_request = None;
                if self
                    .practice
                    .as_ref()
                    .is_some_and(|session| session.phase == Phase::Preparing)
                {
                    self.start_practice_recording();
                }
            }
        } else if self.practice.as_ref().is_some_and(|session| {
            matches!(
                session.phase,
                Phase::Preparing | Phase::Reading | Phase::Checking
            )
        }) && let Some(update) = self.voice.poll()
        {
            self.practice_update(update);
        }
        if self.pronunciation_request.is_none()
            && self
                .practice
                .as_ref()
                .is_some_and(|session| session.should_check(Instant::now()))
        {
            self.practice.as_mut().expect("已检查跟读轮次").checking();
            self.show_practice();
            let update = self.voice.stop();
            self.practice_update(update);
        }
    }

    fn speak_word(&mut self) {
        let Some(word) = self
            .practice
            .as_ref()
            .and_then(|session| session.view().word_card)
            .map(|card| card.word)
        else {
            return;
        };
        self.pronunciation_sequence = self.pronunciation_sequence.wrapping_add(1);
        let request = u64::MAX.saturating_sub(self.pronunciation_sequence);
        let mut resume_recording = false;
        let result = (|| {
            if self
                .practice
                .as_ref()
                .is_some_and(|session| !matches!(session.phase, Phase::Reading | Phase::Failed))
            {
                return Err("准备完成后可播放单词发音".to_owned());
            }
            if self.demo.is_none() {
                self.demo = Some(crate::voice::global::demo::EnglishDemo::new()?);
            }
            if let Some(session) = &mut self.practice
                && session.phase == Phase::Reading
            {
                // 先停止收音再发播放命令，避免 SAPI 在线程切换时被识别成用户跟读。
                self.voice.cancel();
                session.pause_for_pronunciation();
                resume_recording = true;
            }
            self.demo
                .as_mut()
                .expect("已创建 SAPI 播放线程")
                .speak_in_language(
                    request,
                    &word,
                    0,
                    crate::voice::global::practice::lookup::pronunciation_language(
                        &word,
                        self.config.learn_english,
                    ),
                )
        })();
        match result {
            Ok(()) => {
                self.pronunciation_request = Some(request);
                self.demo_ready_at = None;
                if let Some(session) = &mut self.practice {
                    session.set_word_card_hint(Some("播放时暂停收音，已读进度保留".to_owned()));
                }
                self.show_practice();
            }
            Err(error) => {
                if let Some(session) = &mut self.practice {
                    session.set_word_card_hint(Some(error));
                }
                if resume_recording {
                    self.start_practice_recording();
                } else {
                    self.show_practice();
                }
            }
        }
    }

    pub(super) fn start_practice_recording(&mut self) {
        // 译文可能在几秒后才返回；重新验证原输入框，不能在密码框或已切换目标上开麦。
        if !self.practice_target_current(true) {
            self.practice_error(
                "原输入框已变化，跟读尚未开始；恢复输入框后可重试，或按 Esc 取消".to_owned(),
            );
            return;
        }
        self.sink.set_voice_held(true);
        self.show_practice();
        tracing::info!(
            sequence = self.active.as_ref().map(|active| active.sequence),
            "跟读练习请求开启麦克风"
        );
        let expected = self
            .practice
            .as_ref()
            .map_or_else(String::new, PracticeSession::recognition_hint);
        let update = self
            .voice
            .start_practice_in_language(&expected, self.config.target_language());
        self.practice_update(update);
    }

    pub(super) fn practice_update(&mut self, update: VoiceUpdate) {
        let Some(session) = &mut self.practice else {
            return;
        };
        match update {
            VoiceUpdate::Started => {
                tracing::info!(
                    sequence = self.active.as_ref().map(|active| active.sequence),
                    "跟读练习麦克风已启动"
                );
                session.started();
                self.show_practice();
            }
            VoiceUpdate::Interim { text } => {
                if session.interim(&text, Instant::now()) {
                    self.show_practice();
                }
            }
            VoiceUpdate::Final { text } => {
                if session.final_result(&text) {
                    let original = session
                        .completed_original()
                        .expect("完整跟读才交付原话")
                        .to_owned();
                    tracing::info!("跟读练习完成，准备填入原话");
                    self.finish(original);
                } else if session.phase == Phase::Preparing {
                    self.start_practice_recording();
                } else {
                    self.voice.cancel();
                    self.show_practice();
                }
            }
            VoiceUpdate::Error { message } => self.practice_error(message),
            _ => {}
        }
    }

    fn practice_error(&mut self, message: String) {
        self.pronunciation_request = None;
        self.voice.cancel();
        if let Some(demo) = &mut self.demo {
            demo.cancel();
        }
        self.demo_ready_at = None;
        self.game.cancel();
        if let Some(translator) = &mut self.translator {
            translator.cancel();
        }
        if let Some(session) = &mut self.practice {
            session.fail(message);
        }
        tracing::warn!("跟读练习未完成，原话未填入");
        self.show_practice();
    }

    pub(super) fn show_practice(&self) {
        if let (Some(active), Some(session)) = (&self.active, &self.practice) {
            let rect = active
                .target
                .as_ref()
                .map_or_else(empty_rect, |target| target.rect());
            self.sink.show_practice(session.view(), rect);
        }
    }

    pub(super) fn practice_target_current(&self, full: bool) -> bool {
        self.active.as_ref().is_some_and(|active| {
            active.target.as_ref().is_some_and(|target| {
                let result = if full {
                    self.targets.validate(target)
                } else {
                    self.targets.validate_focus(target)
                };
                if let Err(error) = result {
                    tracing::warn!(
                        sequence = active.sequence,
                        full,
                        ?error,
                        "跟读练习目标复核失败"
                    );
                    false
                } else {
                    true
                }
            }) && {
                let current = self.permitted.allowed()
                    && self.hook.healthy()
                    && self.hook.activity_revision() == active.revision;
                if !current {
                    tracing::info!(
                        sequence = active.sequence,
                        full,
                        permitted = self.permitted.allowed(),
                        healthy = self.hook.healthy(),
                        activity_current = self.hook.activity_revision() == active.revision,
                        "跟读练习活动复核失败"
                    );
                }
                current
            }
        })
    }
}
