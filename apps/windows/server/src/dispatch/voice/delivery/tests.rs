//! 回投只交给匹配宿主、等待真实 ACK，失败和丢弃事务不能复活。

use super::{DeliveryBridge, DeliveryFailure};
use qingjian_platform::protocol::{SessionId, VoiceDeliveryCommand, VoiceDeliveryReport};
use std::sync::mpsc;
use std::time::{Duration, Instant};

fn request(bridge: &DeliveryBridge) -> u64 {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Some(VoiceDeliveryCommand::Capture { request }) =
            bridge.poll(SessionId(7), 100, 200, None)
        {
            return request;
        }
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
}

#[test]
fn capture_is_bound_to_session_process_and_thread_and_commit_requires_ack() {
    let bridge = DeliveryBridge::default();
    bridge.poll(SessionId(7), 100, 200, None);
    assert!(bridge.capture(100, 201).unwrap().is_none());
    let peer = bridge.clone();
    let (sender, receiver) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        sender
            .send(
                peer.capture(100, 200)
                    .expect("capture result")
                    .expect("capture"),
            )
            .unwrap();
    });
    let id = request(&bridge);
    assert!(matches!(
        bridge.poll(
            SessionId(8),
            100,
            200,
            Some(VoiceDeliveryReport::Ready { request: id })
        ),
        Some(VoiceDeliveryCommand::Cancel { .. })
    ));
    bridge.poll(
        SessionId(7),
        100,
        200,
        Some(VoiceDeliveryReport::Ready { request: id }),
    );
    let ticket = receiver.recv_timeout(Duration::from_secs(2)).unwrap();
    worker.join().unwrap();
    let insertion = std::thread::spawn(move || ticket.insert("你好"));
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Some(VoiceDeliveryCommand::Commit { request, text, .. }) =
            bridge.poll(SessionId(7), 100, 200, None)
        {
            assert_eq!(request, id);
            assert_eq!(text, "你好");
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    bridge.poll(
        SessionId(7),
        100,
        200,
        Some(VoiceDeliveryReport::Inserted { request: id }),
    );
    assert_eq!(insertion.join().unwrap(), Ok(()));
    assert!(matches!(
        bridge.poll(
            SessionId(7),
            100,
            200,
            Some(VoiceDeliveryReport::Ready { request: id })
        ),
        Some(VoiceDeliveryCommand::Cancel { .. })
    ));
}

#[test]
fn disconnect_revokes_captured_insertion() {
    let bridge = DeliveryBridge::default();
    bridge.poll(SessionId(7), 100, 200, None);
    let peer = bridge.clone();
    let worker = std::thread::spawn(move || peer.capture(100, 200));
    let id = request(&bridge);
    bridge.poll(
        SessionId(7),
        100,
        200,
        Some(VoiceDeliveryReport::Ready { request: id }),
    );
    let ticket = worker.join().unwrap().unwrap().unwrap();
    bridge.close(SessionId(7));
    assert!(!ticket.current());
    assert_eq!(ticket.insert("不能写入"), Err(DeliveryFailure::Rejected));
}

#[test]
fn old_client_never_receives_delivery_or_starts_duplicate_recording() {
    use crate::dispatch::{Router, RouterConfig};
    use qingjian_core::Engine;
    use qingjian_platform::protocol::{ClientMessage, ServerMessage};
    let mut router = Router::new(
        Engine::new(qingjian_dictionary::Dictionary::default()),
        RouterConfig::default(),
    );
    router.external_voice = true;
    assert!(!router.input_settings().voice_input);
    let response = router.handle(ClientMessage::OpenSession {
        session: SessionId(7),
        app: None,
        protocol: 7,
    });
    assert!(matches!(
        response,
        Some(ServerMessage::SessionOpened { .. })
    ));
    let response = router.handle(ClientMessage::SyncMode {
        session: SessionId(7),
    });
    assert!(matches!(response, Some(ServerMessage::ModeSync { .. })));
}

#[test]
fn disconnect_during_commit_and_late_rejection_never_allow_replay() {
    use super::state::{Pending, Phase};
    let bridge = DeliveryBridge::default();
    bridge.0.0.lock().unwrap().pending = Some(Pending {
        request: 42,
        session: SessionId(7),
        process: 100,
        thread: 200,
        phase: Phase::Commit {
            text: "一次".into(),
            deadline_ms: u64::MAX,
        },
    });
    bridge.close(SessionId(7));
    bridge.poll(
        SessionId(7),
        100,
        200,
        Some(VoiceDeliveryReport::Rejected { request: 42 }),
    );
    assert!(matches!(
        bridge.0.0.lock().unwrap().pending.as_ref().unwrap().phase,
        Phase::Unconfirmed
    ));
}

#[test]
fn acknowledged_insertion_remains_successful_after_disconnect() {
    use super::state::{Pending, Phase};
    let bridge = DeliveryBridge::default();
    bridge.0.0.lock().unwrap().pending = Some(Pending {
        request: 42,
        session: SessionId(7),
        process: 100,
        thread: 200,
        phase: Phase::Inserted,
    });
    bridge.close(SessionId(7));
    assert!(matches!(
        bridge.0.0.lock().unwrap().pending.as_ref().unwrap().phase,
        Phase::Inserted
    ));
}

#[test]
fn host_capture_rejection_is_distinct_from_a_missing_client() {
    let bridge = DeliveryBridge::default();
    bridge.poll(SessionId(7), 100, 200, None);
    let peer = bridge.clone();
    let worker = std::thread::spawn(move || peer.capture(100, 200));
    let id = request(&bridge);
    bridge.poll(
        SessionId(7),
        100,
        200,
        Some(VoiceDeliveryReport::Rejected { request: id }),
    );
    assert!(matches!(
        worker.join().unwrap(),
        Err(DeliveryFailure::Rejected)
    ));
}
