//! 全局语音与宿主 TSF 的有界事务桥；共享内存只保存目标身份和当前一次请求。

mod state;
mod ticket;

use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use qingjian_platform::protocol::{SessionId, VoiceDeliveryCommand, VoiceDeliveryReport};

use state::{Pending, Phase, State};
pub(crate) use ticket::{DeliveryFailure, DeliveryTicket};

#[derive(Clone, Default)]
pub(crate) struct DeliveryBridge(Arc<(Mutex<State>, Condvar)>);

impl DeliveryBridge {
    pub(crate) fn poll(
        &self,
        session: SessionId,
        process: u32,
        thread: u32,
        report: Option<VoiceDeliveryReport>,
    ) -> Option<VoiceDeliveryCommand> {
        let (lock, changed) = &*self.0;
        let mut state = lock.lock().ok()?;
        if process == 0 || thread == 0 {
            return None;
        }
        state
            .clients
            .retain(|_, (_, seen)| seen.elapsed() <= Duration::from_secs(2));
        state
            .clients
            .insert((process, thread), (session, Instant::now()));
        if let Some(pending) = state.pending.as_mut().filter(|pending| {
            pending.session == session && pending.process == process && pending.thread == thread
        }) {
            match report {
                Some(VoiceDeliveryReport::Ready { request })
                    if request == pending.request && matches!(pending.phase, Phase::Capturing) =>
                {
                    pending.phase = Phase::Ready
                }
                Some(VoiceDeliveryReport::Inserted { request })
                    if request == pending.request
                        && matches!(pending.phase, Phase::Commit { .. }) =>
                {
                    pending.phase = Phase::Inserted
                }
                Some(VoiceDeliveryReport::Rejected { request })
                    if request == pending.request
                        && matches!(
                            pending.phase,
                            Phase::Capturing | Phase::Ready | Phase::Commit { .. }
                        ) =>
                {
                    pending.phase = Phase::Rejected
                }
                _ => {}
            }
            changed.notify_all();
            return match &pending.phase {
                Phase::Capturing | Phase::Ready => Some(VoiceDeliveryCommand::Capture {
                    request: pending.request,
                }),
                Phase::Commit { text, deadline_ms } => Some(VoiceDeliveryCommand::Commit {
                    request: pending.request,
                    text: text.clone(),
                    deadline_ms: *deadline_ms,
                }),
                Phase::Inserted | Phase::Rejected | Phase::Unconfirmed => {
                    Some(VoiceDeliveryCommand::Cancel {
                        request: pending.request,
                    })
                }
            };
        }
        report.map(|report| VoiceDeliveryCommand::Cancel {
            request: match report {
                VoiceDeliveryReport::Ready { request }
                | VoiceDeliveryReport::Inserted { request }
                | VoiceDeliveryReport::Rejected { request } => request,
            },
        })
    }

    pub(crate) fn capture(
        &self,
        process: u32,
        thread: u32,
    ) -> Result<Option<DeliveryTicket>, DeliveryFailure> {
        let (lock, changed) = &*self.0;
        let mut state = lock.lock().map_err(|_| DeliveryFailure::Rejected)?;
        let Some(&(session, seen)) = state.clients.get(&(process, thread)) else {
            return Ok(None);
        };
        if seen.elapsed() > Duration::from_secs(2) {
            return Ok(None);
        }
        state.next = state.next.checked_add(1).ok_or(DeliveryFailure::Rejected)?;
        let request = state.next;
        state.pending = Some(Pending {
            request,
            session,
            process,
            thread,
            phase: Phase::Capturing,
        });
        let (mut state, _) = changed
            .wait_timeout_while(state, Duration::from_millis(900), |state| {
                state.pending.as_ref().is_some_and(|pending| {
                    pending.request == request && matches!(pending.phase, Phase::Capturing)
                })
            })
            .map_err(|_| DeliveryFailure::Rejected)?;
        if state.pending.as_ref().is_some_and(|pending| {
            pending.request == request && matches!(pending.phase, Phase::Ready)
        }) {
            Ok(Some(DeliveryTicket {
                bridge: self.clone(),
                request,
            }))
        } else {
            let rejected = state.pending.as_ref().is_some_and(|pending| {
                pending.request == request && matches!(pending.phase, Phase::Rejected)
            });
            if state
                .pending
                .as_ref()
                .is_some_and(|pending| pending.request == request)
            {
                state.pending = None;
            }
            if rejected {
                Err(DeliveryFailure::Rejected)
            } else {
                Ok(None)
            }
        }
    }

    pub(crate) fn close(&self, session: SessionId) {
        if let Ok(mut state) = self.0.0.lock() {
            state.clients.retain(|_, (client, _)| *client != session);
            if let Some(pending) = state
                .pending
                .as_mut()
                .filter(|pending| pending.session == session)
            {
                pending.phase = match pending.phase {
                    Phase::Commit { .. } | Phase::Unconfirmed => Phase::Unconfirmed,
                    Phase::Inserted => Phase::Inserted,
                    _ => Phase::Rejected,
                };
            }
            self.0.1.notify_all();
        }
    }
}

impl Default for State {
    fn default() -> Self {
        Self {
            clients: HashMap::new(),
            next: 0,
            pending: None,
        }
    }
}

#[cfg(test)]
mod tests;
