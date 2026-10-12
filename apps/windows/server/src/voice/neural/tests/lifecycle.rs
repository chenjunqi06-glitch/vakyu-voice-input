//! 子进程协议、冷启动松键、取消与看门狗的回归测试；不访问真实麦克风。

use std::cell::RefCell;
use std::io::Cursor;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use qingjian_platform::protocol::VoiceUpdate;

use crate::voice::neural::closing::CloseWatchdog;
use crate::voice::neural::event::HelperEvent;
use crate::voice::neural::process::read_event;
use crate::voice::neural::session::{FINISH_TIMEOUT, START_TIMEOUT, Session};
use crate::voice::neural::worker::reconcile;

#[test]
fn protocol_requires_generation_and_complete_line_without_echoing_body() {
    let event = read_event(&mut Cursor::new(
        b"{\"event\":\"interim\",\"generation\":9,\"text\":\"hello\"}\n",
    ))
    .unwrap();
    assert_eq!(event.generation(), Some(9));
    let error = read_event(&mut Cursor::new(
        b"{\"event\":\"interim\",\"text\":\"private transcript\"}\n",
    ))
    .unwrap_err();
    assert!(!error.contains("private transcript"));
    assert!(read_event(&mut Cursor::new(b"{\"event\":\"ready\"}")).is_err());
    assert!(read_event(&mut Cursor::new(b"")).is_err());
}

#[test]
fn oversized_output_is_bounded_and_rejected() {
    let bytes = vec![b'x'; 1_048_580];
    let mut reader = Cursor::new(bytes);
    assert!(read_event(&mut reader).is_err());
    assert_eq!(reader.position(), 1_048_577);
}

#[test]
fn model_becoming_ready_after_release_does_not_send_start() {
    let mut closing = CloseWatchdog::default();
    let desired = AtomicU64::new(15);
    let mut active = Some(Session::new(7));
    let (updates, results) = mpsc::channel();
    let sent = RefCell::new(Vec::new());
    let send = |command, generation| {
        sent.borrow_mut().push((command, generation));
        Ok(())
    };
    reconcile(&send, &mut active, &updates, &desired, false, &mut closing).unwrap();
    assert!(sent.borrow().is_empty());
    desired.store(14, Ordering::Release);
    reconcile(&send, &mut active, &updates, &desired, true, &mut closing).unwrap();
    assert!(sent.borrow().is_empty());
    assert!(active.is_none());
    assert_eq!(
        results.recv().unwrap(),
        (
            7,
            VoiceUpdate::Final {
                text: String::new()
            }
        )
    );
}

#[test]
fn cancellation_closes_old_generation_without_final_text() {
    let mut closing = CloseWatchdog::default();
    let desired = AtomicU64::new(15);
    let mut active = Some(Session::new(7));
    let (updates, results) = mpsc::channel();
    let sent = RefCell::new(Vec::new());
    let send = |command, generation| {
        sent.borrow_mut().push((command, generation));
        Ok(())
    };
    reconcile(&send, &mut active, &updates, &desired, true, &mut closing).unwrap();
    desired.store(17, Ordering::Release);
    reconcile(&send, &mut active, &updates, &desired, true, &mut closing).unwrap();
    assert_eq!(*sent.borrow(), vec![("start", 7), ("cancel", 7)]);
    assert!(active.is_none());
    assert!(results.try_recv().is_err());
    active = Some(Session::new(8));
    closing.acknowledge(&HelperEvent::Cancelled { generation: 7 });
    reconcile(&send, &mut active, &updates, &desired, true, &mut closing).unwrap();
    assert_eq!(
        *sent.borrow(),
        vec![("start", 7), ("cancel", 7), ("start", 8)]
    );
}

#[test]
fn released_started_session_sends_stop_once_and_waits_for_final() {
    let mut closing = CloseWatchdog::default();
    let desired = AtomicU64::new(15);
    let mut active = Some(Session::new(7));
    let (updates, results) = mpsc::channel();
    let sent = RefCell::new(Vec::new());
    let send = |command, generation| {
        sent.borrow_mut().push((command, generation));
        Ok(())
    };
    reconcile(&send, &mut active, &updates, &desired, true, &mut closing).unwrap();
    desired.store(14, Ordering::Release);
    reconcile(&send, &mut active, &updates, &desired, true, &mut closing).unwrap();
    reconcile(&send, &mut active, &updates, &desired, true, &mut closing).unwrap();
    assert_eq!(*sent.borrow(), vec![("start", 7), ("stop", 7)]);
    assert!(active.unwrap().finishing.is_some());
    assert!(results.try_recv().is_err());
}

#[test]
fn failed_control_keeps_generation_for_visible_failure() {
    let mut closing = CloseWatchdog::default();
    let desired = AtomicU64::new(16);
    let mut session = Session::new(7);
    session.dispatched = Some(Instant::now());
    let mut active = Some(session);
    let (updates, _) = mpsc::channel();
    let send = |_, _| Err("control disconnected".to_owned());
    assert!(reconcile(&send, &mut active, &updates, &desired, true, &mut closing).is_err());
    assert_eq!(active.unwrap().generation, 7);
}

#[test]
fn stale_or_undispatched_results_are_ignored() {
    let old = HelperEvent::Final {
        generation: 6,
        text: "旧录音".to_owned(),
    };
    let current = HelperEvent::Final {
        generation: 7,
        text: "当前录音".to_owned(),
    };
    let mut session = Session::new(7);
    assert!(!session.accepts(&current));
    session.dispatched = Some(Instant::now());
    assert!(!session.accepts(&old));
    assert!(session.accepts(&current));
}

#[test]
fn watchdog_distinguishes_start_and_final_deadlines() {
    let now = Instant::now();
    let mut session = Session::new(7);
    session.dispatched = Some(now);
    assert!(session.timeout(now + START_TIMEOUT).is_some());
    session.started = true;
    assert!(session.timeout(now + START_TIMEOUT).is_none());
    session.finishing = Some(now + FINISH_TIMEOUT);
    assert!(
        session
            .timeout(now + FINISH_TIMEOUT - Duration::from_millis(1))
            .is_none()
    );
    assert!(session.timeout(now + FINISH_TIMEOUT).is_some());
}
