//! 全局后台不可用时，跟读设置不能通过旧 DLL 入口或迟到结果绕过。

use qingjian_core::Engine;
use qingjian_dictionary::Dictionary;
use qingjian_platform::protocol::{
    ClientMessage, ServerMessage, SessionId, VoiceAction, VoiceUpdate,
};

use crate::dispatch::{Router, RouterConfig};

fn router() -> Router {
    let mut router = Router::new(Engine::new(Dictionary::default()), RouterConfig::default());
    router.config.voice_input = true;
    router.handle(ClientMessage::OpenSession {
        session: SessionId(1),
        app: None,
        protocol: 7,
    });
    router
}

#[test]
fn practice_blocks_legacy_fallback_when_global_startup_is_unavailable() {
    let mut router = router();
    router.config.voice_english_practice = true;
    assert!(!router.external_voice);
    assert!(!router.input_settings().voice_input);
    assert!(router.voice_access_error(SessionId(1)).is_some());
    for request in [
        ClientMessage::VoiceStart {
            session: SessionId(1),
        },
        ClientMessage::VoiceControl {
            session: SessionId(1),
            sequence: 1,
            action: VoiceAction::Press,
        },
    ] {
        assert!(matches!(
            router.handle(request),
            Some(ServerMessage::Update {
                voice: Some(VoiceUpdate::Error { .. }),
                ..
            })
        ));
        assert!(router.voice_session.is_none());
    }
}

#[test]
fn ordinary_voice_keeps_legacy_fallback_when_practice_is_disabled() {
    let mut router = router();
    router.config.voice_english_practice = false;
    assert!(!router.external_voice);
    assert!(router.input_settings().voice_input);
    assert!(router.voice_access_error(SessionId(1)).is_none());
    assert!(matches!(
        router.handle(ClientMessage::VoiceControl {
            session: SessionId(1),
            sequence: 1,
            action: VoiceAction::Press,
        }),
        Some(ServerMessage::Update {
            voice: Some(VoiceUpdate::Armed),
            ..
        })
    ));
    assert!(router.voice_session.is_some());
}

#[cfg(windows)]
#[test]
fn enabling_practice_rejects_already_queued_legacy_final_on_stop_and_poll() {
    use crate::voice::TestVoiceCommand;

    for stop in [false, true] {
        let mut router = router();
        let (commands, updates) = router.voice.test_channels();
        assert!(matches!(
            router.start_voice(SessionId(1)),
            ServerMessage::Update {
                voice: Some(VoiceUpdate::Starting),
                ..
            }
        ));
        let generation = commands
            .try_iter()
            .find_map(|command| match command {
                TestVoiceCommand::Start(generation) => Some(generation),
                _ => None,
            })
            .expect("测试通道应收到开始命令，不能打开实际麦克风");
        updates
            .send((
                generation,
                VoiceUpdate::Final {
                    text: "尚未跟读的中文不能交付".to_owned(),
                },
            ))
            .unwrap();
        router.config.voice_english_practice = true;
        let update = if stop {
            match router.stop_voice(SessionId(1)) {
                ServerMessage::Update { voice, .. } => voice,
                _ => panic!("应返回语音终止结果"),
            }
        } else {
            router.poll_voice(SessionId(1))
        };
        assert!(matches!(update, Some(VoiceUpdate::Error { .. })));
        assert!(router.voice_session.is_none());
    }
}

#[test]
fn enabling_practice_expires_an_armed_legacy_round() {
    let mut router = router();
    router.handle(ClientMessage::VoiceControl {
        session: SessionId(1),
        sequence: 1,
        action: VoiceAction::Press,
    });
    assert!(router.voice_session.is_some());
    router.config.voice_english_practice = true;
    router.expire_voice();
    assert!(router.voice_session.is_none());
}
