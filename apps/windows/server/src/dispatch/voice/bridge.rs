//! 稳定 TSF 语音桥：Server 判断长按，序号隔离过期控制，只有新鲜 Poll 才能开麦。

use std::time::{Duration, Instant};

use qingjian_platform::protocol::{ScreenRect, ServerMessage, SessionId, VoiceAction, VoiceUpdate};

use super::session::VoiceSession;
use super::{external_reply, reply};
use crate::dispatch::{RecordingMode, Router};

impl Router {
    pub(in crate::dispatch) fn control_voice(
        &mut self,
        session: SessionId,
        sequence: u64,
        action: VoiceAction,
    ) -> ServerMessage {
        if self.external_voice {
            return external_reply(session);
        }
        // 不能先刷新 lease：迟到的 Poll 不应把已经失联的按住重新变成有效录音。
        self.expire_voice();
        if action == VoiceAction::Press {
            return self.press_voice_bridge(session, sequence);
        }
        if !self
            .voice_session
            .as_ref()
            .is_some_and(|active| active.id == session && active.sequence == Some(sequence))
        {
            if matches!(action, VoiceAction::Poll | VoiceAction::Release)
                && self
                    .sessions
                    .get(&session)
                    .is_some_and(|info| info.voice_sequence == Some(sequence))
            {
                return reply(
                    session,
                    Some(VoiceUpdate::Error {
                        message: "语音会话已结束，请松开 Alt 后重试".to_owned(),
                    }),
                );
            }
            return reply(session, None);
        }
        match action {
            VoiceAction::Press => unreachable!(),
            VoiceAction::Cancel => {
                self.cancel_voice();
                reply(session, None)
            }
            VoiceAction::Release => {
                if self
                    .voice_session
                    .as_ref()
                    .is_some_and(|active| active.pending_since.is_some())
                {
                    self.cancel_voice();
                    reply(
                        session,
                        Some(VoiceUpdate::Final {
                            text: String::new(),
                        }),
                    )
                } else {
                    self.stop_voice_round(session, Some(sequence))
                }
            }
            VoiceAction::Poll => self.poll_voice_bridge(session, sequence),
        }
    }

    fn press_voice_bridge(&mut self, session: SessionId, sequence: u64) -> ServerMessage {
        if let Some(message) = self.voice_admission_error(session) {
            return reply(
                session,
                Some(VoiceUpdate::Error {
                    message: message.to_owned(),
                }),
            );
        }
        if let Some(active) = self.voice_session.as_ref() {
            if active.id != session || active.sequence.is_none() {
                return reply(
                    session,
                    Some(VoiceUpdate::Error {
                        message: "已有其他语音会话正在进行".to_owned(),
                    }),
                );
            }
            if active.sequence == Some(sequence) {
                // 重复 Press 不延长租约或重置长按计时，也不再次打开麦克风。
                return reply(session, active.pending_since.map(|_| VoiceUpdate::Armed));
            }
        }
        let info = self.sessions.get_mut(&session).expect("已检查会话");
        if info.voice_sequence.is_some_and(|last| sequence <= last) {
            return reply(session, None);
        }
        info.voice_sequence = Some(sequence);
        self.cancel_voice();
        self.ensure_focus(session);
        let now = Instant::now();
        self.voice_session = Some(VoiceSession {
            id: session,
            held: true,
            anchor: self.last_rect.unwrap_or(ScreenRect {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            }),
            last_poll: now,
            sequence: Some(sequence),
            pending_since: Some(now),
        });
        // 等待长按时既不预备录音也不显示浮窗；短按 Alt 保持应用原有行为。
        reply(session, Some(VoiceUpdate::Armed))
    }

    fn poll_voice_bridge(&mut self, session: SessionId, sequence: u64) -> ServerMessage {
        let pending = self
            .voice_session
            .as_ref()
            .is_some_and(|active| active.pending_since.is_some());
        // 松开后允许继续敲拼音；最终语音由 DLL 的原插入点桥等待组句完成再交付。
        let error = if pending {
            self.voice_admission_error(session)
        } else {
            self.voice_access_error(session)
        };
        if let Some(message) = error {
            self.cancel_voice();
            return reply(
                session,
                Some(VoiceUpdate::Error {
                    message: message.to_owned(),
                }),
            );
        }
        let active = self.voice_session.as_mut().expect("已校验语音桥序号");
        active.last_poll = Instant::now();
        if let Some(pressed) = active.pending_since {
            let delay =
                Duration::from_millis(u64::from(self.config.voice_hold_ms.clamp(100, 1000)));
            if pressed.elapsed() < delay {
                return reply(session, Some(VoiceUpdate::Armed));
            }
            active.pending_since = None;
            let anchor = active.anchor;
            self.hide_candidate_window();
            let update = self.voice.start();
            if !matches!(update, VoiceUpdate::Error { .. }) {
                self.voice_sink
                    .prepare_voice(anchor, RecordingMode::Dictation);
            }
            self.apply_voice_update(&update);
            return reply(session, Some(update));
        }
        let update = self.poll_voice_round(session, Some(sequence));
        reply(session, update)
    }
}
