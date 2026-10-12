//! 一次原插入点的拥有权；丢弃票据后迟到的回包不能复活事务。

use super::{DeliveryBridge, state::Phase};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub(crate) struct DeliveryTicket {
    pub(super) bridge: DeliveryBridge,
    pub(super) request: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DeliveryFailure {
    Rejected,
    Unconfirmed,
}

impl DeliveryTicket {
    pub(crate) fn current(&self) -> bool {
        self.bridge.0.0.lock().is_ok_and(|state| {
            state.pending.as_ref().is_some_and(|pending| {
                pending.request == self.request && !matches!(pending.phase, Phase::Rejected)
            })
        })
    }

    pub(crate) fn insert(&self, text: &str) -> Result<(), DeliveryFailure> {
        self.insert_with_timeout(text, Duration::from_millis(1900))
    }

    fn insert_with_timeout(&self, text: &str, wait: Duration) -> Result<(), DeliveryFailure> {
        let (lock, changed) = &*self.bridge.0;
        let mut state = lock.lock().map_err(|_| DeliveryFailure::Rejected)?;
        let pending = state
            .pending
            .as_mut()
            .filter(|pending| pending.request == self.request)
            .ok_or(DeliveryFailure::Unconfirmed)?;
        match pending.phase {
            Phase::Ready => {}
            Phase::Inserted => return Ok(()),
            Phase::Commit { .. } | Phase::Unconfirmed => return Err(DeliveryFailure::Unconfirmed),
            _ => return Err(DeliveryFailure::Rejected),
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| DeliveryFailure::Rejected)?
            .as_millis() as u64;
        pending.phase = Phase::Commit {
            text: text.to_owned(),
            deadline_ms: now + 1500,
        };
        let (state, _) = changed
            .wait_timeout_while(state, wait, |state| {
                state.pending.as_ref().is_some_and(|pending| {
                    pending.request == self.request && matches!(pending.phase, Phase::Commit { .. })
                })
            })
            .map_err(|_| DeliveryFailure::Unconfirmed)?;
        match state
            .pending
            .as_ref()
            .filter(|pending| pending.request == self.request)
            .map(|pending| &pending.phase)
        {
            Some(Phase::Inserted) => Ok(()),
            Some(Phase::Rejected) => Err(DeliveryFailure::Rejected),
            _ => Err(DeliveryFailure::Unconfirmed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DeliveryFailure, DeliveryTicket};
    use crate::dispatch::voice::delivery::{
        DeliveryBridge,
        state::{Pending, Phase},
    };
    use qingjian_platform::protocol::SessionId;
    use std::time::Duration;

    #[test]
    fn missing_commit_ack_is_unconfirmed_and_never_a_safe_rejection() {
        let bridge = DeliveryBridge::default();
        bridge.0.0.lock().unwrap().pending = Some(Pending {
            request: 1,
            session: SessionId(7),
            process: 100,
            thread: 200,
            phase: Phase::Ready,
        });
        let ticket = DeliveryTicket { bridge, request: 1 };
        assert_eq!(
            ticket.insert_with_timeout("你好", Duration::ZERO),
            Err(DeliveryFailure::Unconfirmed)
        );
        assert_eq!(
            ticket.insert_with_timeout("不能重复写", Duration::ZERO),
            Err(DeliveryFailure::Unconfirmed)
        );
        assert!(matches!(
            ticket
                .bridge
                .0
                .0
                .lock()
                .unwrap()
                .pending
                .as_ref()
                .unwrap()
                .phase,
            Phase::Commit { .. }
        ));
    }
}

impl Drop for DeliveryTicket {
    fn drop(&mut self) {
        if let Ok(mut state) = self.bridge.0.0.lock() {
            if state
                .pending
                .as_ref()
                .is_some_and(|pending| pending.request == self.request)
            {
                state.pending = None;
            }
            self.bridge.0.1.notify_all();
        }
    }
}
