//! 关闭采集确认必须独立于输入会话，旧确认不能解除新会话的保护。

use std::io::Cursor;
use std::sync::atomic::AtomicU64;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::voice::neural::closing::{CLOSE_TIMEOUT, CloseWatchdog};
use crate::voice::neural::event::HelperEvent;
use crate::voice::neural::process::read_event;
use crate::voice::neural::session::Session;
use crate::voice::neural::worker::reconcile;

#[test]
fn missing_cancel_ack_times_out_even_after_session_is_removed() {
    let desired = AtomicU64::new(16);
    let mut session = Session::new(7);
    session.dispatched = Some(Instant::now());
    let mut active = Some(session);
    let mut closing = CloseWatchdog::default();
    let (updates, _) = mpsc::channel();
    reconcile(
        &|_, _| Ok(()),
        &mut active,
        &updates,
        &desired,
        true,
        &mut closing,
    )
    .unwrap();
    assert!(active.is_none());
    assert!(closing.check(Instant::now()).is_ok());
    // run_connected 将此错误返回进程所有者，HelperProcess::drop 随即 kill 并 wait。
    assert!(closing.check(Instant::now() + CLOSE_TIMEOUT).is_err());
}

#[test]
fn stop_requires_capture_ack_separately_from_final_deadline() {
    let now = Instant::now();
    let mut closing = CloseWatchdog::default();
    closing.arm(7, false, now);
    assert!(
        closing
            .check(now + CLOSE_TIMEOUT - Duration::from_millis(1))
            .is_ok()
    );
    assert!(closing.check(now + CLOSE_TIMEOUT).is_err());
    assert!(closing.acknowledge(&HelperEvent::Stopped { generation: 7 }));
    assert!(closing.check(now + CLOSE_TIMEOUT).is_ok());
}

#[test]
fn old_ack_does_not_release_new_generation_watchdog() {
    let now = Instant::now();
    let mut closing = CloseWatchdog::default();
    closing.arm(7, true, now);
    closing.arm(8, true, now);
    closing.acknowledge(&HelperEvent::Cancelled { generation: 7 });
    assert!(closing.check(now + CLOSE_TIMEOUT).is_err());
    closing.acknowledge(&HelperEvent::Cancelled { generation: 8 });
    assert!(closing.check(now + CLOSE_TIMEOUT).is_ok());
}

#[test]
fn stop_ack_does_not_clear_a_later_cancel_request() {
    let now = Instant::now();
    let mut closing = CloseWatchdog::default();
    closing.arm(7, false, now);
    closing.arm(7, true, now);
    closing.acknowledge(&HelperEvent::Stopped { generation: 7 });
    assert!(closing.check(now + CLOSE_TIMEOUT).is_err());
    closing.acknowledge(&HelperEvent::Cancelled { generation: 7 });
    assert!(closing.check(now + CLOSE_TIMEOUT).is_ok());
}

#[test]
fn repeated_requests_cannot_extend_a_stuck_microphone_deadline() {
    let now = Instant::now();
    let mut closing = CloseWatchdog::default();
    closing.arm(7, true, now);
    closing.arm(7, true, now + Duration::from_secs(1));
    assert!(closing.check(now + CLOSE_TIMEOUT).is_err());
}

#[test]
fn capture_ack_protocol_requires_generation() {
    let stopped = read_event(&mut Cursor::new(
        b"{\"event\":\"stopped\",\"generation\":7}\n",
    ))
    .unwrap();
    let cancelled = read_event(&mut Cursor::new(
        b"{\"event\":\"cancelled\",\"generation\":8}\n",
    ))
    .unwrap();
    assert!(matches!(stopped, HelperEvent::Stopped { generation: 7 }));
    assert!(matches!(
        cancelled,
        HelperEvent::Cancelled { generation: 8 }
    ));
    assert!(read_event(&mut Cursor::new(b"{\"event\":\"cancelled\"}\n")).is_err());
}
