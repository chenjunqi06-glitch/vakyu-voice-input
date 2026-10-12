//! 全局后台接管后，运行中的新旧 DLL 均不能再开麦、收语音浮窗或取得转写。

use std::sync::{Arc, Mutex};

use qingjian_core::Engine;
use qingjian_dictionary::Dictionary;
use qingjian_platform::protocol::{
    ClientMessage, ScreenRect, ServerMessage, SessionId, VoiceAction, VoiceUpdate,
};

use crate::dispatch::{Router, RouterConfig, VoiceSink};

struct Capture(Arc<Mutex<usize>>);

impl VoiceSink for Capture {
    fn show_voice(&self, _text: String, _rect: ScreenRect) {
        *self.0.lock().unwrap() += 1;
    }

    fn hide_voice(&self) {
        *self.0.lock().unwrap() += 1;
    }
}

fn router() -> (Router, Arc<Mutex<usize>>) {
    let mut router = Router::new(Engine::new(Dictionary::default()), RouterConfig::default());
    let calls = Arc::new(Mutex::new(0));
    router.set_voice_sink(Box::new(Capture(calls.clone())));
    router.external_voice = true;
    router.config.voice_input = true;
    router.handle(ClientMessage::OpenSession {
        session: SessionId(1),
        app: None,
        protocol: 7,
    });
    (router, calls)
}

#[test]
fn existing_and_new_dlls_receive_disabled_local_voice_without_changing_user_setting() {
    let (mut router, _) = router();
    assert!(matches!(
        router.handle(ClientMessage::SyncMode { session: SessionId(1) }),
        Some(ServerMessage::ModeSync { input, .. }) if !input.voice_input
    ));
    assert!(matches!(
        router.handle(ClientMessage::OpenSession {
            session: SessionId(2), app: None, protocol: 7,
        }),
        Some(ServerMessage::SessionOpened { input, .. }) if !input.voice_input
    ));
    assert!(router.config.voice_input);
}

#[test]
fn old_voice_commands_cannot_start_stop_or_read_the_global_recording() {
    let (mut router, calls) = router();
    let session = SessionId(1);
    for message in [
        ClientMessage::VoiceStart { session },
        ClientMessage::VoiceStop { session },
        ClientMessage::VoiceControl {
            session,
            sequence: 1,
            action: VoiceAction::Press,
        },
        ClientMessage::VoiceControl {
            session,
            sequence: 1,
            action: VoiceAction::Poll,
        },
        ClientMessage::VoiceControl {
            session,
            sequence: 1,
            action: VoiceAction::Release,
        },
        ClientMessage::VoiceControl {
            session,
            sequence: 1,
            action: VoiceAction::Cancel,
        },
    ] {
        assert!(matches!(
            router.handle(message),
            Some(ServerMessage::Update {
                voice: Some(VoiceUpdate::Error { .. }),
                ..
            })
        ));
    }
    assert!(matches!(
        router.handle(ClientMessage::Poll { session }),
        Some(ServerMessage::Update { voice: None, .. })
    ));
    router.handle(ClientMessage::VoiceCancel { session });
    assert!(router.voice_session.is_none());
    assert_eq!(*calls.lock().unwrap(), 0);
}

#[test]
fn old_dll_focus_privacy_disconnect_and_hide_do_not_close_the_global_bubble() {
    let (mut router, calls) = router();
    let session = SessionId(1);
    router.ensure_focus(session);
    router.bind_connection(session, 1);
    router.handle(ClientMessage::HideCandidates { session });
    router.handle(ClientMessage::Privacy {
        session,
        private: true,
    });
    router.handle(ClientMessage::Commit { session });
    router.handle(ClientMessage::ImeSwitched { session });
    router.handle_connected(
        ClientMessage::OpenSession {
            session,
            app: None,
            protocol: 7,
        },
        2,
    );
    router.disconnect_session(session, 1);
    assert!(router.sessions.contains_key(&session));
    router.disconnect_session(session, 2);
    assert!(!router.sessions.contains_key(&session));
    router.tick();
    assert_eq!(*calls.lock().unwrap(), 0);
    assert!(router.external_voice);
}
