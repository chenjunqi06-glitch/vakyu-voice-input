//! 钩子线程局部状态；不会跨线程借用 Win32 句柄。

use super::{event::TriggerEvent, machine::Machine, outcome::Outcome, shared::Shared};
use std::sync::{Arc, atomic::Ordering, mpsc::Sender};

pub(super) struct Runtime {
    pub machine: Machine,

    pub shared: Arc<Shared>,

    pub events: Sender<TriggerEvent>,
}

impl Runtime {
    pub fn publish(&self, outcome: &Outcome) {
        let (sequence, key) = self
            .machine
            .active
            .as_ref()
            .map_or((0, 0), |active| (active.sequence, active.key.vk));
        self.shared.key.store(key, Ordering::Release);
        self.shared.sequence.store(sequence, Ordering::Release);
        if let Some(event) = outcome.event {
            let _ = self.events.send(event);
        }
    }
}
