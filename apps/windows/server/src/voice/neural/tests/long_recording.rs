//! 长时间按住只继续当前代次，松键与取消仍立即关闭采集。

use std::cell::RefCell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::voice::neural::closing::CloseWatchdog;
use crate::voice::neural::session::Session;
use crate::voice::neural::worker::reconcile;

#[test]
fn long_recording_stays_open_until_release_or_cancel() {
    for release in [true, false] {
        let mut closing = CloseWatchdog::default();
        let desired = AtomicU64::new(15);
        let mut session = Session::new(7);
        session.dispatched = Some(Instant::now() - Duration::from_secs(600));
        session.started = true;
        let mut active = Some(session);
        let (updates, results) = mpsc::channel();
        let sent = RefCell::new(Vec::new());
        let send = |command, generation| {
            sent.borrow_mut().push((command, generation));
            Ok(())
        };
        reconcile(&send, &mut active, &updates, &desired, true, &mut closing).unwrap();
        reconcile(&send, &mut active, &updates, &desired, true, &mut closing).unwrap();
        assert!(sent.borrow().is_empty(), "长录音不能自动停止或重新开麦");
        assert!(active.as_ref().unwrap().timeout(Instant::now()).is_none());
        assert!(results.try_recv().is_err());

        desired.store(if release { 14 } else { 16 }, Ordering::Release);
        reconcile(&send, &mut active, &updates, &desired, true, &mut closing).unwrap();
        reconcile(&send, &mut active, &updates, &desired, true, &mut closing).unwrap();
        assert_eq!(
            *sent.borrow(),
            vec![(if release { "stop" } else { "cancel" }, 7)]
        );
        if release {
            assert!(active.unwrap().finishing.is_some());
        } else {
            assert!(active.is_none());
        }
    }
}
