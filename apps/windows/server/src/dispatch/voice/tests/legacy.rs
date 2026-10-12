//! 不连接麦克风，验证浮窗生命周期、会话隔离和失联取消。

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use qingjian_core::Engine;
use qingjian_dictionary::Dictionary;
use qingjian_platform::protocol::{
    ClientMessage, Frame, ScreenRect, ServerMessage, SessionId, VoiceUpdate,
};

use crate::dispatch::voice::session::VoiceSession;
use crate::dispatch::voice::sink::VoiceSink;
use crate::dispatch::{Router, RouterConfig};

struct Capture(Arc<Mutex<Vec<String>>>);

impl VoiceSink for Capture {
    fn show_voice(&self, text: String, _rect: ScreenRect) {
        self.0.lock().unwrap().push(text);
    }

    fn hide_voice(&self) {
        self.0.lock().unwrap().push("<hide>".to_owned());
    }
}

fn active_router() -> (Router, Arc<Mutex<Vec<String>>>) {
    let engine = Engine::new(Dictionary::parse("你\tni\t100\n").unwrap());
    let mut router = Router::new(engine, RouterConfig::default());
    let output = Arc::new(Mutex::new(Vec::new()));
    router.set_voice_sink(Box::new(Capture(output.clone())));
    router.focused = Some(SessionId(1));
    router.voice_session = Some(VoiceSession {
        id: SessionId(1),
        held: true,
        anchor: ScreenRect {
            left: 50,
            top: 50,
            right: 51,
            bottom: 70,
        },
        last_poll: Instant::now(),
        sequence: None,
        pending_since: None,
    });
    (router, output)
}

#[test]
fn late_partial_and_final_never_reopen_a_released_bubble() {
    let (mut router, output) = active_router();
    router.apply_voice_update(&VoiceUpdate::Interim {
        text: "今天".to_owned(),
    });
    router.voice_session.as_mut().unwrap().held = false;
    router.voice_sink.hide_voice();
    router.apply_voice_update(&VoiceUpdate::Interim {
        text: "今天去吃饭".to_owned(),
    });
    router.apply_voice_update(&VoiceUpdate::Final {
        text: "今天去吃饭。".to_owned(),
    });
    assert_eq!(*output.lock().unwrap(), ["今天", "<hide>", "<hide>"]);
    assert!(router.voice_session.is_none());
}

#[test]
fn another_client_cannot_stop_or_refresh_the_recording() {
    let (mut router, output) = active_router();
    let last_poll = router.voice_session.as_ref().unwrap().last_poll;
    assert_eq!(router.poll_voice(SessionId(2)), None);
    assert_eq!(
        router.stop_voice(SessionId(2)),
        ServerMessage::Update {
            session: SessionId(2),
            frame: Frame::default(),
            voice: None,
        }
    );
    router.cancel_voice_for(SessionId(2));
    assert_eq!(router.voice_session.as_ref().unwrap().last_poll, last_poll);
    assert!(router.voice_session.as_ref().unwrap().held);
    assert!(output.lock().unwrap().is_empty());
}

#[test]
fn focus_change_privacy_and_missing_heartbeat_end_voice() {
    let (mut router, _) = active_router();
    router.ensure_focus(SessionId(2));
    assert!(router.voice_session.is_none());
    let (mut router, _) = active_router();
    router.handle(ClientMessage::Privacy {
        session: SessionId(1),
        private: true,
    });
    assert!(router.voice_session.is_none());
    let (mut router, _) = active_router();
    router.voice_session.as_mut().unwrap().last_poll = Instant::now() - Duration::from_secs(3);
    router.expire_voice();
    assert!(router.voice_session.is_none());
}

#[test]
fn unregistered_or_disabled_start_is_rejected_before_audio_is_opened() {
    let (mut router, _) = active_router();
    assert!(matches!(
        router.start_voice(SessionId(99)),
        ServerMessage::Update {
            voice: Some(VoiceUpdate::Error { .. }),
            ..
        }
    ));
    router.config.voice_input = false;
    assert!(matches!(
        router.start_voice(SessionId(1)),
        ServerMessage::Update {
            voice: Some(VoiceUpdate::Error { .. }),
            ..
        }
    ));
}
