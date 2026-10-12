//! 全局语音只借用当前宿主的 TSF 插入点；此通道不启动或停止录音。

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use qingjian_platform::protocol::{ServerMessage, VoiceDeliveryCommand, VoiceDeliveryReport};
use windows::Win32::System::Threading::{GetCurrentProcessId, GetCurrentThreadId};

use super::TextService_Impl;
use crate::com::edit::{VoiceAnchor, request_voice_anchor, request_voice_commit};
use crate::com::log::log;

#[derive(Default)]
pub(super) struct DeliveryState {
    request: Cell<Option<u64>>,

    anchor: RefCell<Option<Rc<VoiceAnchor>>>,

    committing: Cell<bool>,
}

impl DeliveryState {
    pub(super) fn clear(&self) {
        if let Some(anchor) = self.anchor.borrow_mut().take() {
            anchor.cancel();
        }
        self.request.set(None);
        self.committing.set(false);
    }

    fn report(&self) -> Option<VoiceDeliveryReport> {
        let request = self.request.get()?;
        let anchors = self.anchor.borrow();
        let Some(anchor) = anchors.as_ref() else {
            return Some(VoiceDeliveryReport::Rejected { request });
        };
        if anchor.committed() {
            Some(VoiceDeliveryReport::Inserted { request })
        } else if anchor.cancelled() {
            Some(VoiceDeliveryReport::Rejected { request })
        } else if anchor.ready() {
            Some(VoiceDeliveryReport::Ready { request })
        } else {
            None
        }
    }
}

impl TextService_Impl {
    pub(super) fn delivery_tick(&self, foreground: bool, sync_tick: bool) -> bool {
        if !self
            .input_settings
            .get()
            .is_some_and(|input| input.voice_delivery)
        {
            self.delivery.clear();
            return false;
        }
        let urgent = self.delivery.request.get().is_some()
            && (self.delivery.committing.get()
                || self
                    .delivery
                    .anchor
                    .borrow()
                    .as_ref()
                    .is_none_or(|anchor| !anchor.ready() || anchor.cancelled()));
        if !(urgent || foreground && sync_tick) {
            return false;
        }
        let result = {
            let Ok(mut guard) = self.engine.try_borrow_mut() else {
                return false;
            };
            let Some(client) = guard.as_mut() else {
                return false;
            };
            client.voice_delivery_poll(
                unsafe { GetCurrentProcessId() },
                unsafe { GetCurrentThreadId() },
                self.delivery.report(),
            )
        };
        let reply = match result {
            Ok(reply) => reply,
            Err(error) => {
                log(&format!("全局语音回投同步失败: {error}"));
                self.delivery.clear();
                self.disconnect();
                return true;
            }
        };
        let ServerMessage::VoiceDelivery {
            input,
            english,
            indicator,
            command,
            ..
        } = reply
        else {
            return true;
        };
        self.apply_input_settings(input);
        self.indicator_state.set(indicator);
        if foreground {
            self.adopt_mode(english);
        }
        match command {
            Some(VoiceDeliveryCommand::Capture { request })
                if self.delivery.request.get() != Some(request) =>
            {
                self.delivery.clear();
                self.delivery.request.set(Some(request));
                if !foreground
                    || self.shared.composing()
                    || self.shared.has_composition()
                    || self.shared.translating()
                {
                    return true;
                }
                let Some(manager) = self.thread_mgr.borrow().clone() else {
                    return true;
                };
                if !unsafe { manager.IsThreadFocus() }.is_ok_and(|focus| focus.as_bool()) {
                    return true;
                }
                let Ok(context) =
                    (unsafe { manager.GetFocus().and_then(|document| document.GetTop()) })
                else {
                    return true;
                };
                let anchor = VoiceAnchor::new(context, manager);
                *self.delivery.anchor.borrow_mut() = Some(anchor.clone());
                if let Err(error) =
                    request_voice_anchor(anchor.clone(), self.client_id.get(), self.engine.clone())
                {
                    anchor.cancel();
                    log(&format!("全局语音保存插入点失败: {error}"));
                }
            }
            Some(VoiceDeliveryCommand::Commit {
                request,
                text,
                deadline_ms,
            }) if self.delivery.request.get() == Some(request) => {
                self.delivery.committing.set(true);
                if let Some(anchor) = self.delivery.anchor.borrow().clone() {
                    anchor.set_deadline(deadline_ms);
                    if !anchor.committed() && !anchor.focused() {
                        anchor.cancel();
                    }
                    if !anchor.cancelled()
                        && !anchor.queued()
                        && !anchor.committed()
                        && !self.shared.composing()
                        && !self.shared.has_composition()
                    {
                        if let Err(error) = request_voice_commit(
                            anchor.clone(),
                            self.client_id.get(),
                            self.shared.clone(),
                            text,
                        ) {
                            anchor.cancel();
                            log(&format!("全局语音 TSF 插入失败: {error}"));
                        }
                    }
                }
            }
            Some(VoiceDeliveryCommand::Cancel { request })
                if self.delivery.request.get() == Some(request) =>
            {
                self.delivery.clear()
            }
            Some(VoiceDeliveryCommand::Commit { request, .. })
                if self.delivery.request.get() != Some(request) =>
            {
                self.delivery.clear();
                self.delivery.request.set(Some(request));
            }
            None => self.delivery.clear(),
            _ => {}
        }
        true
    }
}
