//! 按住说话的生命周期：限定会话归属，停止即收窗，轮询异步交付最后的文字。

mod bridge;
pub(crate) mod delivery;
#[cfg(windows)]
mod global;
mod noop;
mod practice_view;
mod recording_mode;
pub(super) mod session;
#[cfg(windows)]
mod shared_sink;
mod sink;

#[cfg(test)]
mod tests;

use std::time::{Duration, Instant};

use qingjian_platform::protocol::{Frame, ScreenRect, ServerMessage, SessionId, VoiceUpdate};

pub use self::noop::NoopVoiceSink;
pub use self::practice_view::{PracticeAction, PracticeView, PracticeWordCard};
pub use self::recording_mode::RecordingMode;
use self::session::VoiceSession;
pub use self::sink::VoiceSink;
use super::Router;

const VOICE_LEASE: Duration = Duration::from_secs(2);

const PRACTICE_REQUIRES_GLOBAL: &str = "英语跟读后台未就绪，本次中文不会填入";

impl Router {
    /// Server 正式启动后预热语音模型，保持所有测试构造无后台副作用。
    pub fn prewarm_voice(&mut self) {
        if self.config.voice_input && !self.external_voice {
            self.voice.prewarm();
        }
    }

    pub(super) fn start_voice(&mut self, session: SessionId) -> ServerMessage {
        if self.external_voice {
            return external_reply(session);
        }
        self.expire_voice();
        let error = self.voice_admission_error(session).or_else(|| {
            self.voice_session.as_ref().and_then(|active| {
                (active.id != session || active.sequence.is_some())
                    .then_some("已有其他语音会话正在进行")
            })
        });
        if let Some(message) = error {
            return reply(
                session,
                Some(VoiceUpdate::Error {
                    message: message.to_owned(),
                }),
            );
        }
        self.cancel_voice();
        let anchor = self.last_rect.unwrap_or(ScreenRect {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        });
        self.ensure_focus(session);
        self.hide_candidate_window();
        let update = self.voice.start();
        if !matches!(update, VoiceUpdate::Error { .. }) {
            self.voice_session = Some(VoiceSession {
                id: session,
                held: true,
                anchor,
                last_poll: Instant::now(),
                sequence: None,
                pending_since: None,
            });
            self.voice_sink
                .prepare_voice(anchor, RecordingMode::Dictation);
        }
        reply(session, Some(update))
    }

    fn voice_admission_error(&self, session: SessionId) -> Option<&'static str> {
        self.voice_access_error(session).or_else(|| {
            (!self.engine.composition().is_empty() || self.translation.is_some())
                .then_some("请先完成当前选词")
        })
    }

    fn voice_access_error(&self, session: SessionId) -> Option<&'static str> {
        if !self.sessions.contains_key(&session) {
            Some("语音会话未打开")
        } else if !self.config.voice_input {
            Some("语音输入已关闭")
        } else if self.config.voice_english_practice {
            // 全局入口启动失败时也不能回退成不经过跟读门禁的旧语音流程。
            Some(PRACTICE_REQUIRES_GLOBAL)
        } else if self.sessions.get(&session).is_some_and(|info| info.private) {
            Some("私密输入框不可使用语音")
        } else {
            None
        }
    }

    pub(super) fn stop_voice(&mut self, session: SessionId) -> ServerMessage {
        if self.external_voice {
            return external_reply(session);
        }
        self.stop_voice_round(session, None)
    }

    fn stop_voice_round(&mut self, session: SessionId, sequence: Option<u64>) -> ServerMessage {
        if self.config.voice_english_practice {
            self.cancel_voice_for(session);
            return reply(
                session,
                Some(VoiceUpdate::Error {
                    message: PRACTICE_REQUIRES_GLOBAL.to_owned(),
                }),
            );
        }
        let Some(active) = self
            .voice_session
            .as_mut()
            .filter(|active| active.id == session && active.sequence == sequence)
        else {
            return reply(session, None);
        };
        if !active.held {
            return reply(session, Some(VoiceUpdate::Stopping));
        }
        active.held = false;
        active.last_poll = Instant::now();
        // 先收窗；识别器还需要处理最后一小段声音，不能阻塞按键线程。
        self.voice_sink.hide_voice();
        let update = self.voice.stop();
        self.apply_voice_update(&update);
        reply(session, Some(update))
    }

    pub(super) fn poll_voice(&mut self, session: SessionId) -> Option<VoiceUpdate> {
        if self.external_voice {
            return None;
        }
        self.poll_voice_round(session, None)
    }

    fn poll_voice_round(
        &mut self,
        session: SessionId,
        sequence: Option<u64>,
    ) -> Option<VoiceUpdate> {
        if self.config.voice_english_practice
            && self
                .voice_session
                .as_ref()
                .is_some_and(|active| active.id == session && active.sequence == sequence)
        {
            self.cancel_voice_for(session);
            return Some(VoiceUpdate::Error {
                message: PRACTICE_REQUIRES_GLOBAL.to_owned(),
            });
        }
        let active = self
            .voice_session
            .as_mut()
            .filter(|active| active.id == session && active.sequence == sequence)?;
        active.last_poll = Instant::now();
        let update = self.voice.poll()?;
        self.apply_voice_update(&update);
        Some(update)
    }

    fn apply_voice_update(&mut self, update: &VoiceUpdate) {
        match update {
            VoiceUpdate::Interim { text } => {
                if let Some(active) = &self.voice_session
                    && active.held
                {
                    self.voice_sink.show_voice(
                        text.clone(),
                        active.anchor,
                        RecordingMode::Dictation,
                    );
                }
            }
            VoiceUpdate::Final { .. } => {
                self.voice_sink.hide_voice();
                self.voice_session = None;
            }
            VoiceUpdate::Error { message } => {
                tracing::warn!(%message, "语音识别失败");
                if let Some(active) = self.voice_session.take()
                    && active.held
                {
                    self.voice_sink.voice_error(
                        message.clone(),
                        active.anchor,
                        RecordingMode::Dictation,
                    );
                } else {
                    self.voice_sink.hide_voice();
                }
            }
            VoiceUpdate::Started => {
                if let Some(active) = &self.voice_session
                    && active.held
                {
                    self.voice_sink.show_voice(
                        String::new(),
                        active.anchor,
                        RecordingMode::Dictation,
                    );
                }
            }
            VoiceUpdate::Armed | VoiceUpdate::Starting | VoiceUpdate::Stopping => {}
        }
    }

    pub(super) fn cancel_voice_for(&mut self, session: SessionId) {
        if self
            .voice_session
            .as_ref()
            .is_some_and(|active| active.id == session)
        {
            self.cancel_voice();
        }
    }

    pub(super) fn cancel_legacy_voice_for(&mut self, session: SessionId) {
        if self
            .voice_session
            .as_ref()
            .is_some_and(|active| active.sequence.is_none())
        {
            self.cancel_voice_for(session);
        }
    }

    pub(super) fn cancel_voice(&mut self) {
        if self.external_voice {
            return;
        }
        self.voice.cancel();
        self.voice_session = None;
        self.voice_sink.hide_voice();
    }

    /// 更新前必须先撤销后台的录音和上屏许可，不能只清掉旧 DLL 会话。
    pub(super) fn prepare_voice_update(&mut self) -> bool {
        #[cfg(windows)]
        if let Some(global) = &self.global_voice
            && !global.prepare_update()
        {
            tracing::error!("全局语音没有确认停止，取消本次组件更新");
            return false;
        }
        self.cancel_voice();
        true
    }

    pub(super) fn expire_voice(&mut self) {
        // 宿主崩溃、断线或卡住不能留下后台录音。
        if self
            .voice_session
            .as_ref()
            .is_some_and(|active| active.last_poll.elapsed() > VOICE_LEASE)
            || ((!self.config.voice_input || self.config.voice_english_practice)
                && self.voice_session.is_some())
        {
            self.cancel_voice();
        }
    }
}

fn external_reply(session: SessionId) -> ServerMessage {
    reply(
        session,
        Some(VoiceUpdate::Error {
            message: "语音已由全局后台接管，请直接按住语音键使用".to_owned(),
        }),
    )
}

fn reply(session: SessionId, voice: Option<VoiceUpdate>) -> ServerMessage {
    ServerMessage::Update {
        session,
        frame: Frame::default(),
        voice,
    }
}
