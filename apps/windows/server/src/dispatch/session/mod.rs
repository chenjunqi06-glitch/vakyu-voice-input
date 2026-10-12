//! 会话：DLL 每个应用线程一条，Server 记宿主应用；焦点在哪个会话，组句就属于谁。

mod info;

use qingjian_platform::protocol::{ClientMessage, Frame, ServerMessage, SessionId};

pub(super) use self::info::SessionInfo;
use super::Router;

impl Router {
    /// 连接已被同名会话的新连接替代后，旧队列中的语音与关闭命令不再有控制权。
    pub(crate) fn handle_connected(
        &mut self,
        message: ClientMessage,
        connection: u64,
    ) -> Option<ServerMessage> {
        match &message {
            ClientMessage::VoiceControl { session, .. }
            | ClientMessage::VoiceDeliveryPoll { session, .. }
            | ClientMessage::VoiceStart { session }
            | ClientMessage::VoiceStop { session }
                if self.connection_replaced(*session, connection) =>
            {
                return Some(ServerMessage::Update {
                    session: *session,
                    frame: Frame::default(),
                    voice: None,
                });
            }
            ClientMessage::VoiceCancel { session } | ClientMessage::CloseSession { session }
                if session.0 != 0 && self.connection_replaced(*session, connection) =>
            {
                return None;
            }
            _ => {}
        }
        let opened = match &message {
            ClientMessage::OpenSession { session, .. } => Some(*session),
            _ => None,
        };
        let reply = self.handle(message);
        if let Some(session) = opened {
            self.bind_connection(session, connection);
        }
        reply
    }

    fn connection_replaced(&self, session: SessionId, connection: u64) -> bool {
        self.sessions
            .get(&session)
            .is_some_and(|info| info.connection != Some(connection))
    }

    pub(crate) fn bind_connection(&mut self, session: SessionId, connection: u64) {
        if let Some(info) = self.sessions.get_mut(&session) {
            info.connection = Some(connection);
        }
    }

    pub(crate) fn disconnect_session(&mut self, session: SessionId, connection: u64) {
        if self
            .sessions
            .get(&session)
            .is_some_and(|info| info.connection == Some(connection))
        {
            self.handle(ClientMessage::CloseSession { session });
        }
    }

    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// 聚焦会话所在应用的 exe 名；DLL 没报时为 `None`。
    pub(super) fn focused_app(&self) -> Option<&str> {
        self.focused
            .and_then(|session| self.sessions.get(&session))
            .and_then(|info| info.app.as_deref())
    }

    pub(super) fn ensure_focus(&mut self, session: SessionId) {
        if self.focused != Some(session) {
            self.cancel_voice();
            self.reset_composition();
            self.focused = Some(session);
            let app = self.focused_app().map(str::to_owned);
            self.engine.set_application(app);
            let private = self.focused_private();
            self.engine.set_private(private);
        }
    }

    /// 当前聚焦的会话在私密输入框里（Engine 不学不记不发云端）。
    pub fn is_private(&self) -> bool {
        self.engine.is_private()
    }

    fn focused_private(&self) -> bool {
        self.focused
            .and_then(|session| self.sessions.get(&session))
            .is_some_and(|info| info.private)
    }

    /// DLL 报来该会话的输入框私密与否变了：记下；是当前会话就立刻让 Engine 进 / 出私密。
    pub(super) fn set_privacy(&mut self, session: SessionId, private: bool) {
        if let Some(info) = self.sessions.get_mut(&session) {
            info.private = private;
        }
        if self.focused == Some(session) {
            self.engine.set_private(private);
        }
    }

    /// 焦点离开：把缓冲区原样交出并清组句。组句不属于 `session` 时只清不交，别把 A 应用的拼音落进 B。
    pub(super) fn commit_raw_for(&mut self, session: SessionId) -> Option<String> {
        let text = (self.focused == Some(session) && !self.engine.composition().is_empty())
            .then(|| self.engine.take_raw());
        self.reset_composition();
        text
    }

    /// 清掉组句、展示状态、在飞的云联想与翻译评审，收起候选窗口。
    pub(super) fn reset_composition(&mut self) {
        self.engine.break_chain();
        self.engine.clear();
        self.cancel_prediction();
        self.stop_rescoring();
        self.composed = None;
        self.translation = None;
        self.pending_selection = None;
        self.sentence = None;
        self.notice = None;
        self.highlight = 0;
        self.navigated = false;
        self.hide_candidate_window();
    }
}
