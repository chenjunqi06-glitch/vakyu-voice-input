//! 从 TSF 收键或前台线程轮询准备语音，所有入口共用焦点和文档检查。

use std::time::Instant;

use windows::Win32::UI::TextServices::ITfContext;

use super::super::TextService_Impl;
use super::{hold::Hold, keys::AltKeys, other_key_down};
use crate::com::context::keyboard_disabled;
use crate::com::edit::{VoiceAnchor, request_voice_anchor};
use crate::com::log::log;

impl TextService_Impl {
    /// 裸 Alt 属于 WM_SYSKEYDOWN，部分宿主不会把它转给 TSF，轮询补齐按下边沿。
    pub(super) fn poll_voice_press(&self, foreground: bool) {
        let key = self.voice_key();
        if self.voice.hold.get() == Hold::Idle && self.voice.key.replace(key) != key {
            // 修改左右键配置不构成一次物理按下，已经按住的新键须先松开。
            self.voice
                .press
                .borrow_mut()
                .interrupt(self.voice_alt_down());
        }
        let foreground = foreground
            && self.thread_mgr.borrow().as_ref().is_some_and(|manager| {
                unsafe { manager.IsThreadFocus() }.is_ok_and(|focus| focus.as_bool())
            });
        let pressed = self
            .voice
            .press
            .borrow_mut()
            .sample(self.voice_alt_down(), foreground);
        if !pressed || !foreground || self.voice.hold.get() != Hold::Idle {
            return;
        }
        let context = self
            .thread_mgr
            .borrow()
            .as_ref()
            .and_then(|manager| unsafe {
                manager
                    .GetFocus()
                    .and_then(|document| document.GetTop())
                    .ok()
            });
        if let Some(context) = context {
            self.arm_voice(&context, "轮询");
        } else {
            self.reject_voice_press("没有可输入的文档");
        }
    }

    pub(super) fn arm_voice(&self, context: &ITfContext, source: &str) {
        if self.voice.hold.get() != Hold::Idle {
            return;
        }
        self.voice.consume_release.set(false);
        let Some(manager) = self.thread_mgr.borrow().clone() else {
            self.reject_voice_press("线程管理器未就绪");
            return;
        };
        let reason = if !self.voice.profile.active() {
            Some("当前键盘输入法不是 Vakyu")
        } else if !unsafe { manager.IsThreadFocus() }.is_ok_and(|focus| focus.as_bool()) {
            Some("当前 TSF 线程没有键盘焦点")
        } else if !unsafe { manager.GetFocus().and_then(|document| document.GetTop()) }
            .is_ok_and(|focused| focused == *context)
        {
            Some("输入文档已失去焦点")
        } else if self.shared.composing() || self.shared.has_composition() {
            Some("正在键盘组句")
        } else if self.shared.translating() {
            Some("正在翻译选区")
        } else if self.poll_timer.borrow().is_none() {
            Some("语音轮询未就绪")
        } else if !self
            .input_settings
            .get()
            .is_some_and(|input| input.voice_input)
        {
            Some("语音未开启或服务尚未确认支持")
        } else if keyboard_disabled(context) {
            Some("输入框禁用了文本服务")
        } else if other_key_down() {
            Some("Alt 组合键")
        } else {
            None
        };
        if let Some(reason) = reason {
            self.reject_voice_press(reason);
            return;
        }

        let Some(sequence) = self.voice.sequence.get().checked_add(1) else {
            self.reject_voice_press("语音序号已耗尽，请重新启用输入法");
            return;
        };
        let input = self.input_settings.get().expect("已确认语音配置");
        self.voice.sequence.set(sequence);
        self.voice.key.set(input.voice_key);
        self.voice.bridge.set(input.voice_bridge_version >= 1);
        self.voice.control_sent.set(false);
        let mut hold = self.voice.hold.get();
        hold.arm(Instant::now(), false, true);
        let target = VoiceAnchor::new(context.clone(), manager);
        *self.voice.target.borrow_mut() = Some(target.clone());
        self.voice.hold.set(hold);
        log(&format!(
            "语音检测到 Alt 按下，来源={source}，侧={}，等候长按与插入点",
            AltKeys::read().side()
        ));
        if let Err(error) = request_voice_anchor(target, self.client_id.get(), self.engine.clone())
        {
            log(&format!("请求语音插入点失败: {error}"));
            self.cancel_voice();
        }
    }

    fn reject_voice_press(&self, reason: &str) {
        self.voice.key.set(self.voice_key());
        self.voice.hold.set(Hold::Blocked);
        log(&format!("语音忽略本次 Alt：{reason}"));
    }
}
