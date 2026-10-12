//! 语音薄桥的长按判定、代次归属与断线回归；真实控制通道不启动麦克风。

use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use qingjian_core::Engine;
use qingjian_dictionary::Dictionary;
use qingjian_platform::protocol::{
    ClientMessage, ServerMessage, SessionId, VoiceAction, VoiceUpdate,
};

use crate::dispatch::{Router, RouterConfig};
use crate::voice::TestVoiceCommand;

pub(super) fn router() -> (Router, Receiver<TestVoiceCommand>) {
    let engine = Engine::new(Dictionary::parse("你\tni\t100\n").unwrap());
    let mut router = Router::new(engine, RouterConfig::default());
    for id in [1, 2] {
        router.handle(ClientMessage::OpenSession {
            session: SessionId(id),
            app: None,
            protocol: 7,
        });
    }
    let (commands, _updates) = router.voice.test_channels();
    (router, commands)
}

pub(super) fn control(
    router: &mut Router,
    session: u64,
    sequence: u64,
    action: VoiceAction,
) -> Option<VoiceUpdate> {
    let reply = router.handle(ClientMessage::VoiceControl {
        session: SessionId(session),
        sequence,
        action,
    });
    let Some(ServerMessage::Update { voice, .. }) = reply else {
        panic!("每条 VoiceControl 必须回 Update");
    };
    voice
}

pub(super) fn starts(commands: &Receiver<TestVoiceCommand>) -> usize {
    commands
        .try_iter()
        .filter(|command| matches!(command, TestVoiceCommand::Start(_)))
        .count()
}

#[test]
fn short_press_release_never_requests_a_microphone() {
    let (mut router, commands) = router();
    assert_eq!(
        control(&mut router, 1, 10, VoiceAction::Press),
        Some(VoiceUpdate::Armed)
    );
    assert_eq!(
        control(&mut router, 1, 10, VoiceAction::Release),
        Some(VoiceUpdate::Final {
            text: String::new()
        })
    );
    assert_eq!(starts(&commands), 0);
    assert!(router.voice_session.is_none());
    assert_eq!(control(&mut router, 1, 10, VoiceAction::Press), None);
    assert!(router.voice_session.is_none());
}

#[test]
fn only_a_fresh_poll_starts_after_the_current_configured_threshold() {
    let (mut router, commands) = router();
    control(&mut router, 1, 10, VoiceAction::Press);
    router.voice_session.as_mut().unwrap().pending_since =
        Some(Instant::now() - Duration::from_millis(250));
    router.config.voice_hold_ms = 500;
    router.tick();
    assert_eq!(starts(&commands), 0, "Server 定时器不能擅自开麦");
    assert_eq!(
        control(&mut router, 1, 10, VoiceAction::Poll),
        Some(VoiceUpdate::Armed)
    );
    router.config.voice_hold_ms = 180;
    assert_eq!(
        control(&mut router, 1, 10, VoiceAction::Poll),
        Some(VoiceUpdate::Starting)
    );
    assert_eq!(starts(&commands), 1);
}

#[test]
fn stale_controls_and_other_sessions_cannot_end_or_steal_a_new_round() {
    let (mut router, commands) = router();
    control(&mut router, 1, 10, VoiceAction::Press);
    control(&mut router, 1, 11, VoiceAction::Press);
    let last_poll = router.voice_session.as_ref().unwrap().last_poll;
    for action in [VoiceAction::Poll, VoiceAction::Release, VoiceAction::Cancel] {
        assert_eq!(control(&mut router, 1, 10, action), None);
        assert_eq!(control(&mut router, 2, 11, action), None);
    }
    assert!(matches!(
        control(&mut router, 2, 12, VoiceAction::Press),
        Some(VoiceUpdate::Error { .. })
    ));
    let active = router.voice_session.as_ref().unwrap();
    assert_eq!(active.sequence, Some(11));
    assert_eq!(active.last_poll, last_poll);
    assert_eq!(starts(&commands), 0);
}

#[test]
fn expired_poll_cannot_resurrect_an_abandoned_press() {
    let (mut router, commands) = router();
    control(&mut router, 1, 10, VoiceAction::Press);
    let active = router.voice_session.as_mut().unwrap();
    active.pending_since = Some(Instant::now() - Duration::from_secs(3));
    active.last_poll = Instant::now() - Duration::from_secs(3);
    assert!(matches!(
        control(&mut router, 1, 10, VoiceAction::Poll),
        Some(VoiceUpdate::Error { .. })
    ));
    assert!(router.voice_session.is_none());
    assert!(matches!(
        control(&mut router, 1, 10, VoiceAction::Release),
        Some(VoiceUpdate::Error { .. })
    ));
    assert_eq!(control(&mut router, 1, 9, VoiceAction::Poll), None);
    assert_eq!(starts(&commands), 0);
}

#[test]
fn disconnect_cancels_immediately_but_old_connection_cannot_cancel_reconnect() {
    let (mut router, commands) = router();
    router.bind_connection(SessionId(1), 2);
    control(&mut router, 1, 10, VoiceAction::Press);
    router.disconnect_session(SessionId(1), 1);
    assert!(router.voice_session.is_some());
    router.disconnect_session(SessionId(1), 2);
    assert!(router.voice_session.is_none());
    assert!(!router.sessions.contains_key(&SessionId(1)));
    assert_eq!(starts(&commands), 0);
}

#[test]
fn legacy_messages_cannot_receive_armed_or_control_a_bridge_round() {
    let (mut router, commands) = router();
    control(&mut router, 1, 10, VoiceAction::Press);
    assert_eq!(router.poll_voice(SessionId(1)), None);
    assert!(matches!(
        router.stop_voice(SessionId(1)),
        ServerMessage::Update { voice: None, .. }
    ));
    router.handle(ClientMessage::VoiceCancel {
        session: SessionId(1),
    });
    assert!(router.voice_session.is_some());
    assert!(matches!(
        router.start_voice(SessionId(1)),
        ServerMessage::Update {
            voice: Some(VoiceUpdate::Error { .. }),
            ..
        }
    ));
    assert_eq!(starts(&commands), 0);
}

#[test]
fn prepare_update_cancels_pending_voice_and_replies_without_transcript() {
    let (mut router, commands) = router();
    control(&mut router, 1, 10, VoiceAction::Press);
    assert!(matches!(
        router.handle(ClientMessage::PrepareUpdate),
        Some(ServerMessage::Update {
            session: SessionId(0),
            voice: None,
            ..
        })
    ));
    assert!(router.voice_session.is_none());
    assert_eq!(starts(&commands), 0);
}
