//! 结束录音期间继续打字与断线重连不能把语音交给错误的输入周期。

use std::time::{Duration, Instant};

use qingjian_platform::protocol::{
    ClientMessage, KeyEvent, ServerMessage, SessionId, VoiceAction, VoiceUpdate,
};

use super::bridge::{control, router, starts};
use crate::voice::TestVoiceCommand;

#[test]
fn typing_after_release_keeps_the_final_voice_result_for_the_anchor() {
    let (mut router, _) = router();
    let (commands, updates) = router.voice.test_channels();
    control(&mut router, 1, 10, VoiceAction::Press);
    router.voice_session.as_mut().unwrap().pending_since =
        Some(Instant::now() - Duration::from_millis(200));
    assert_eq!(
        control(&mut router, 1, 10, VoiceAction::Poll),
        Some(VoiceUpdate::Starting)
    );
    let generation = commands
        .try_iter()
        .find_map(|command| match command {
            TestVoiceCommand::Start(generation) => Some(generation),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        control(&mut router, 1, 10, VoiceAction::Release),
        Some(VoiceUpdate::Stopping)
    );
    router.handle(ClientMessage::Key {
        session: SessionId(1),
        event: KeyEvent::new(u32::from(b'N'), Some('n'), Default::default()),
    });
    assert!(!router.engine.composition().is_empty());
    let final_update = VoiceUpdate::Final {
        text: "今天去吃饭。".to_owned(),
    };
    updates.send((generation, final_update.clone())).unwrap();
    assert_eq!(
        control(&mut router, 1, 10, VoiceAction::Poll),
        Some(final_update)
    );
    assert!(!router.engine.composition().is_empty());
}

#[test]
fn old_connection_eof_after_reopen_keeps_new_pending_session() {
    let (mut router, commands) = router();
    router.bind_connection(SessionId(1), 1);
    control(&mut router, 1, 10, VoiceAction::Press);
    router.handle(ClientMessage::OpenSession {
        session: SessionId(1),
        app: None,
        protocol: 7,
    });
    router.bind_connection(SessionId(1), 2);
    assert_eq!(
        control(&mut router, 1, 11, VoiceAction::Press),
        Some(VoiceUpdate::Armed)
    );
    router.disconnect_session(SessionId(1), 1);
    assert_eq!(router.voice_session.as_ref().unwrap().sequence, Some(11));
    assert_eq!(starts(&commands), 0);
}

#[test]
fn privacy_or_focus_change_cancels_pending_before_any_audio_request() {
    let (mut router, commands) = router();
    control(&mut router, 1, 10, VoiceAction::Press);
    router.handle(ClientMessage::Privacy {
        session: SessionId(1),
        private: true,
    });
    assert!(router.voice_session.is_none());
    router.handle(ClientMessage::Privacy {
        session: SessionId(1),
        private: false,
    });
    control(&mut router, 1, 11, VoiceAction::Press);
    router.ensure_focus(SessionId(2));
    assert!(router.voice_session.is_none());
    assert_eq!(starts(&commands), 0);
}

#[test]
fn replaced_connection_delayed_voice_or_close_cannot_control_new_round() {
    let (mut router, commands) = router();
    router.bind_connection(SessionId(1), 1);
    control(&mut router, 1, 10, VoiceAction::Press);
    router.handle_connected(
        ClientMessage::OpenSession {
            session: SessionId(1),
            app: None,
            protocol: 7,
        },
        2,
    );
    control(&mut router, 1, 11, VoiceAction::Press);
    let last_poll = router.voice_session.as_ref().unwrap().last_poll;
    for action in [
        VoiceAction::Press,
        VoiceAction::Poll,
        VoiceAction::Release,
        VoiceAction::Cancel,
    ] {
        let reply = router.handle_connected(
            ClientMessage::VoiceControl {
                session: SessionId(1),
                sequence: 11,
                action,
            },
            1,
        );
        assert!(matches!(
            reply,
            Some(ServerMessage::Update { voice: None, .. })
        ));
    }
    for message in [
        ClientMessage::VoiceStart {
            session: SessionId(1),
        },
        ClientMessage::VoiceStop {
            session: SessionId(1),
        },
        ClientMessage::VoiceCancel {
            session: SessionId(1),
        },
        ClientMessage::CloseSession {
            session: SessionId(1),
        },
    ] {
        router.handle_connected(message, 1);
    }
    assert_eq!(router.voice_session.as_ref().unwrap().sequence, Some(11));
    assert_eq!(router.voice_session.as_ref().unwrap().last_poll, last_poll);
    assert!(router.sessions.contains_key(&SessionId(1)));
    assert_eq!(starts(&commands), 0);
}

#[test]
fn temporary_profile_notification_and_installer_flush_remain_compatible() {
    let (mut router, commands) = router();
    router.bind_connection(SessionId(1), 2);
    control(&mut router, 1, 10, VoiceAction::Press);
    router.handle_connected(
        ClientMessage::ImeSwitched {
            session: SessionId(1),
        },
        99,
    );
    assert!(router.voice_session.is_none());
    router.last_flush = Instant::now() - Duration::from_secs(1);
    let before = router.last_flush;
    assert_eq!(
        router.handle_connected(
            ClientMessage::CloseSession {
                session: SessionId(0)
            },
            99
        ),
        None
    );
    assert!(router.last_flush > before);
    assert_eq!(starts(&commands), 0);
}
