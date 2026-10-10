//! 按住 Alt 的语音输入：录音与浮窗在 Server，DLL 只捕获长按并在松开后提交最终文本。

mod hold;
mod keys;
mod press;
mod profile;
mod trigger;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Instant;

use windows::Win32::Foundation::LPARAM;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LMENU, VK_MENU, VK_RMENU};
use windows::Win32::UI::TextServices::ITfContext;
use windows::core::Ref;

use qingjian_platform::VoiceKey;
use qingjian_platform::protocol::{VoiceAction, VoiceUpdate};

use self::hold::Hold;
use self::keys::AltKeys;
use self::press::Press;
use self::profile::KeyboardProfile;
use super::TextService_Impl;
use crate::com::edit::{VoiceAnchor, request_voice_commit};
use crate::com::log::log;

#[derive(Default)]
pub(super) struct VoiceState {
    hold: Cell<Hold>,

    press: RefCell<Press>,

    profile: KeyboardProfile,

    /// 单次锚点固定协议与按键配置；配置热更新从下一次按住开始生效。
    key: Cell<VoiceKey>,

    bridge: Cell<bool>,

    sequence: Cell<u64>,

    control_sent: Cell<bool>,

    target: RefCell<Option<Rc<VoiceAnchor>>>,

    /// 已完成的识别结果在用户松 Alt / 结束后续键盘组句前一直保留。
    final_text: RefCell<Option<String>>,

    /// OnTestKeyUp 中已停止录音；OnKeyUp 仍须返回吃键，避免应用打开 Alt 菜单。
    consume_release: Cell<bool>,
}

impl TextService_Impl {
    pub(super) fn voice_key_down(&self, pic: &Ref<ITfContext>, vk: u32, lparam: LPARAM) {
        if !is_alt(vk) {
            // 极少数宿主漏送 Alt key-up；紧接着键入也视为已松开，保留刚才的语音。
            if !self.voice_alt_down()
                && matches!(self.voice.hold.get(), Hold::Recording | Hold::HoldingFinal)
            {
                self.voice_key_up(u32::from(VK_MENU.0));
            }
            if self.voice.hold.get().cancel_on_key() {
                log("语音因 Alt 组合键取消");
                self.cancel_voice();
            } else if self.voice_alt_down() && self.voice.hold.get() == Hold::Idle {
                // 第二个键可能先于轮询到达；即使 Alt 事件被宿主略过，也不能误录 Alt+Tab。
                self.voice.press.borrow_mut().key_down(true);
                self.voice.key.set(self.voice_key());
                self.voice.hold.set(Hold::Blocked);
            }
            return;
        }
        if !AltKeys::matches(self.voice_key(), vk, (lparam.0 >> 24) & 1 != 0) {
            return;
        }
        let pressed = self
            .voice
            .press
            .borrow_mut()
            .key_down((lparam.0 >> 30) & 1 != 0);
        if pressed && let Ok(context) = pic.ok() {
            self.arm_voice(context, "TSF 按键");
        }
    }

    /// 测试阶段就停止录音，保证应用未接收 Alt key-up 时也能立即收起浮窗。
    pub(super) fn voice_key_up(&self, vk: u32) -> bool {
        if !is_alt(vk) {
            return false;
        }
        // 两侧一起按时，松开其中一侧仍在录音。
        if self.voice_alt_down() {
            return self.voice.hold.get() == Hold::Recording
                || self.voice.hold.get() == Hold::HoldingFinal;
        }
        self.voice.press.borrow_mut().release();
        let mut hold = self.voice.hold.get();
        let consume = matches!(hold, Hold::Recording | Hold::HoldingFinal)
            || self.voice.consume_release.get();
        self.voice.consume_release.set(consume);
        let stop = if self.voice.bridge.get() && self.voice.control_sent.get() {
            hold.release_control(Instant::now())
        } else {
            hold.release(Instant::now())
        };
        self.voice.hold.set(hold);
        if stop {
            log("语音 Alt 已松开，上报松键并等待最终结果");
            self.stop_voice();
        } else if hold == Hold::AwaitingCommit {
            self.commit_voice();
        } else if hold == Hold::Idle
            && let Some(target) = self.voice.target.borrow_mut().take()
        {
            log("语音 Alt 短按结束，未启动录音");
            target.cancel();
        }
        consume
    }

    pub(super) fn cancel_voice(&self) {
        self.voice.consume_release.set(false);
        let mut hold = self.voice.hold.get();
        if !matches!(hold, Hold::Idle | Hold::Blocked) {
            log("语音会话取消");
        }
        let down = self.voice_alt_down();
        self.voice.key.set(self.voice_key());
        self.voice.press.borrow_mut().interrupt(down);
        let cancel = hold.interrupt(down);
        let control = self.voice.control_sent.replace(false);
        self.voice.hold.set(hold);
        if let Some(target) = self.voice.target.borrow_mut().take() {
            target.cancel();
        }
        self.voice.final_text.borrow_mut().take();
        if (cancel || control)
            && let Ok(mut guard) = self.engine.try_borrow_mut()
            && let Some(client) = guard.as_mut()
        {
            let result = if self.voice.bridge.get() {
                client
                    .voice_control(self.voice.sequence.get(), VoiceAction::Cancel)
                    .map(|_| ())
            } else {
                client.voice_cancel()
            };
            if let Err(error) = result {
                log(&format!("取消语音失败: {error}"));
                *guard = None;
            }
        }
    }

    pub(super) fn cancel_voice_chord(&self) {
        if self.voice.hold.get().cancel_on_key() {
            log("语音因保留组合键取消");
            self.cancel_voice();
        }
    }

    /// 返回语音是否占用当前轮询；这里只做短 IPC，录音初始化与识别在 Server 后台线程。
    pub(super) fn voice_tick(&self, foreground: bool) -> bool {
        self.poll_voice_press(foreground);
        let hold = self.voice.hold.get();
        if hold == Hold::Idle {
            return false;
        }
        if hold == Hold::Blocked {
            if !self.voice_alt_down() {
                self.voice.hold.set(Hold::Idle);
            }
            return false;
        }
        let reason = if !foreground || !self.voice_target_focused() {
            Some("输入焦点已变化或插入点不可用")
        } else if !self.voice.profile.active() {
            Some("当前键盘输入法已切离 Vakyu")
        } else if self
            .input_settings
            .get()
            .is_some_and(|input| !input.voice_input)
        {
            Some("语音设置已关闭")
        } else if hold.cancel_on_key() && self.voice_alt_down() && other_key_down() {
            Some("检测到 Alt 组合键")
        } else if hold.timed_out(Instant::now()) {
            Some("等候最终结果超时")
        } else {
            None
        };
        if let Some(reason) = reason {
            log(&format!("语音取消原因：{reason}"));
            self.cancel_voice();
            return false;
        }
        if !self.voice_alt_down() && hold.cancel_on_key() {
            self.voice_key_up(u32::from(VK_MENU.0));
            return self.voice_occupies_poll();
        }
        if matches!(hold, Hold::Pending(_))
            && self
                .voice
                .target
                .borrow()
                .as_ref()
                .is_some_and(|target| target.ready())
        {
            if self.voice.bridge.get() {
                if !self.voice.control_sent.get() {
                    self.press_voice_control();
                }
            } else if hold.ready(Instant::now()) {
                self.start_voice();
            }
        }
        if self.voice.hold.get().server_active()
            || (self.voice.control_sent.get() && matches!(self.voice.hold.get(), Hold::Pending(_)))
        {
            let result = {
                let Ok(mut guard) = self.engine.try_borrow_mut() else {
                    return true;
                };
                guard.as_mut().map(|client| {
                    if self.voice.bridge.get() {
                        client.voice_control(self.voice.sequence.get(), VoiceAction::Poll)
                    } else {
                        client.poll_update()
                    }
                })
            };
            match result {
                Some(Ok(reply)) => {
                    self.voice_update(reply.voice);
                    // 新桥控制回复的空帧不表示翻译结束，候选仍由普通 Poll 读取。
                    if !self.voice.bridge.get()
                        && self.shared.translating()
                        && reply.frame.is_empty()
                    {
                        self.shared.set_translating(false);
                        self.shared.hide_candidates();
                    }
                }
                Some(Err(error)) => {
                    log(&format!("语音轮询失败: {error}"));
                    self.cancel_voice();
                    self.disconnect();
                }
                None => self.cancel_voice(),
            }
        }
        if self.voice.hold.get() == Hold::AwaitingCommit {
            self.commit_voice();
            // 键盘组句继续拉云结果，不能被缓存的语音结果停住。
            return false;
        }
        self.voice_occupies_poll()
    }

    fn voice_occupies_poll(&self) -> bool {
        self.voice.hold.get().occupies_poll(
            self.voice.bridge.get(),
            self.shared.composing(),
            self.shared.translating(),
        )
    }

    fn press_voice_control(&self) {
        if !self.ensure_connected() {
            self.cancel_voice();
            return;
        }
        let Some(input) = self.input_settings.get().filter(|input| input.voice_input) else {
            self.cancel_voice();
            return;
        };
        if input.voice_bridge_version == 0 {
            // 重连可能接到旧服务，只在确认新能力后发送它认识的控制包。
            self.voice.bridge.set(false);
            return;
        }
        if !matches!(self.voice.hold.get(), Hold::Pending(_)) || !self.voice_target_focused() {
            self.cancel_voice();
            return;
        }
        self.voice.control_sent.set(true);
        log(&format!(
            "语音桥报告按住，序号={}",
            self.voice.sequence.get()
        ));
        let result = self
            .engine
            .borrow_mut()
            .as_mut()
            .map(|client| client.voice_control(self.voice.sequence.get(), VoiceAction::Press));
        match result {
            Some(Ok(reply)) => self.voice_update(reply.voice),
            Some(Err(error)) => {
                log(&format!("语音桥按住请求失败: {error}"));
                self.cancel_voice();
                self.disconnect();
            }
            None => self.cancel_voice(),
        }
    }

    pub(super) fn voice_anchor_ready(&self, anchor: &Rc<VoiceAnchor>) {
        if self.voice.bridge.get()
            && !self.voice.control_sent.get()
            && matches!(self.voice.hold.get(), Hold::Pending(_))
            && self.voice_alt_down()
            && self
                .voice
                .target
                .borrow()
                .as_ref()
                .is_some_and(|target| Rc::ptr_eq(target, anchor))
        {
            self.press_voice_control();
        }
    }

    fn start_voice(&self) {
        if !self.ensure_connected()
            || !self
                .input_settings
                .get()
                .is_some_and(|input| input.voice_input)
        {
            self.cancel_voice();
            return;
        }
        self.key_tap.cancel();
        self.voice.hold.set(Hold::Recording);
        log("语音长按已确认，向服务请求录音");
        let result = self
            .engine
            .borrow_mut()
            .as_mut()
            .map(|client| client.voice_start());
        match result {
            Some(Ok(update)) => self.voice_update(update),
            Some(Err(error)) => {
                log(&format!("启动语音失败: {error}"));
                self.cancel_voice();
                self.disconnect();
            }
            None => self.cancel_voice(),
        }
    }

    fn stop_voice(&self) {
        let result = self.engine.borrow_mut().as_mut().map(|client| {
            if self.voice.bridge.get() {
                client
                    .voice_control(self.voice.sequence.get(), VoiceAction::Release)
                    .map(|reply| reply.voice)
            } else {
                client.voice_stop()
            }
        });
        match result {
            Some(Ok(update)) => self.voice_update(update),
            Some(Err(error)) => {
                log(&format!("停止语音失败: {error}"));
                self.cancel_voice();
                self.disconnect();
            }
            None => self.cancel_voice(),
        }
    }

    fn voice_target_focused(&self) -> bool {
        self.voice
            .target
            .borrow()
            .as_ref()
            .is_some_and(|target| target.focused())
    }

    fn commit_voice(&self) {
        let target = self.voice.target.borrow().clone();
        let Some(target) = target else { return };
        if target.committed() {
            log("语音最终文字的宿主插入请求已完成");
            self.voice.hold.set(if self.voice_alt_down() {
                Hold::Blocked
            } else {
                Hold::Idle
            });
            self.voice.target.borrow_mut().take();
            self.voice.final_text.borrow_mut().take();
            return;
        }
        if target.queued() || self.shared.composing() || self.shared.has_composition() {
            return;
        }
        let Some(text) = self.voice.final_text.borrow().clone() else {
            return;
        };
        if text.trim().is_empty() {
            self.cancel_voice();
            return;
        }
        if let Err(error) =
            request_voice_commit(target, self.client_id.get(), self.shared.clone(), text)
        {
            log(&format!("语音上屏请求失败: {error}"));
            self.cancel_voice();
        }
    }

    fn voice_update(&self, update: Option<VoiceUpdate>) {
        match update {
            Some(VoiceUpdate::Final { text }) => {
                self.voice.control_sent.set(false);
                let mut hold = self.voice.hold.get();
                if hold.received_final() {
                    log("语音已收到最终结果");
                    *self.voice.final_text.borrow_mut() = Some(text);
                    self.voice.hold.set(hold);
                    if hold == Hold::AwaitingCommit {
                        self.commit_voice();
                    }
                }
            }
            Some(VoiceUpdate::Error { message }) => {
                log(&format!("语音识别失败: {message}"));
                self.cancel_voice();
            }
            // 中间结果只由 Server 浮窗显示，避免反复改动用户正在编辑的正文。
            Some(update @ (VoiceUpdate::Starting | VoiceUpdate::Started)) => {
                let mut hold = self.voice.hold.get();
                hold.recording_started();
                self.voice.hold.set(hold);
                self.key_tap.cancel();
                if update == VoiceUpdate::Started {
                    log("语音麦克风已开始录音");
                }
            }
            Some(VoiceUpdate::Armed | VoiceUpdate::Stopping | VoiceUpdate::Interim { .. })
            | None => {}
        }
    }

    pub(super) fn voice_key(&self) -> VoiceKey {
        if self.voice.hold.get() == Hold::Idle {
            self.input_settings
                .get()
                .map_or(VoiceKey::Alt, |input| input.voice_key)
        } else {
            self.voice.key.get()
        }
    }

    pub(super) fn voice_alt_down(&self) -> bool {
        AltKeys::read().selected_down(self.voice_key())
    }
}

fn is_alt(vk: u32) -> bool {
    [VK_MENU.0, VK_LMENU.0, VK_RMENU.0]
        .into_iter()
        .any(|alt| vk == u32::from(alt))
}

/// 系统有时直接截走 Tab/F4，TSF 看不到该键；在轮询中补查可避免持续录音。
fn other_key_down() -> bool {
    (0x01..=0xFE).any(|vk| !is_alt(vk) && unsafe { GetAsyncKeyState(vk as i32) < 0 })
}
