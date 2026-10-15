//! 全局语音事件循环；录音时仅做快速焦点校验，昂贵的输入框验证在开麦前与收尾后进行。

mod demonstration;
mod practice;

use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender};
use std::time::{Duration, Instant};

use crate::voice::translation::Translator;
use qingjian_platform::protocol::{ScreenRect, VoiceUpdate};

use crate::dispatch::VoiceSink;
use crate::voice::VoiceManager;

use super::active::Active;
use super::command::Command;
use super::config::VoiceConfig;
use super::demo::EnglishDemo;
use super::game::PracticeGame;
use super::permission::Permission;
use super::practice::{
    PracticeSession,
    lifecycle::{ExitEvent, should_close},
};
use super::target::TargetContext;
use super::trigger::{HookThread, TriggerConfig, TriggerEvent};

const TICK: Duration = Duration::from_millis(20);

pub(super) fn run(
    sink: Box<dyn VoiceSink>,
    config: VoiceConfig,
    commands: Receiver<Command>,
    permitted: Arc<Permission>,
    ready: SyncSender<Result<(), String>>,
    delivery: crate::dispatch::voice::delivery::DeliveryBridge,
) {
    let targets = match TargetContext::new(delivery) {
        Ok(targets) => targets,
        Err(error) => {
            let _ = ready.send(Err(error.user_message().to_owned()));
            return;
        }
    };
    let hook = match HookThread::start(config.trigger) {
        Ok(hook) => hook,
        Err(message) => {
            let _ = ready.send(Err(message));
            return;
        }
    };
    if ready.send(Ok(())).is_err() {
        return;
    }
    let game = PracticeGame::load_for_language(config.learn_english);
    let mut controller = Controller {
        sink,
        targets,
        hook,
        voice: VoiceManager::default(),
        permitted,
        config,
        active: None,
        practice: None,
        translator: None,
        demo: None,
        demo_ready_at: None,
        pronunciation_request: None,
        pronunciation_sequence: 0,
        game,
        last_meter_update: Instant::now(),
        paused: false,
    };
    if controller.config.trigger.enabled {
        controller.voice.prewarm();
    }
    controller.prewarm_demo();
    tracing::info!(
        enabled = controller.config.trigger.enabled,
        english_practice = controller.config.english_practice,
        learn_english = controller.config.learn_english,
        practice_demo = controller.config.practice_demo,
        key = ?controller.config.trigger.key,
        "全局语音已启用，无需宿主加载新版输入法组件"
    );
    loop {
        match commands.recv_timeout(TICK) {
            Ok(Command::Shutdown) | Err(RecvTimeoutError::Disconnected) => break,
            Ok(Command::Pause(reply)) => {
                controller.paused = true;
                controller.cancel();
                if controller
                    .hook
                    .configure(TriggerConfig {
                        enabled: false,
                        ..controller.config.trigger
                    })
                    .is_ok()
                {
                    let _ = reply.send(());
                }
            }
            Ok(Command::Configure(config, revision)) => {
                controller.cancel();
                if controller.config.learn_english != config.learn_english {
                    controller.game = PracticeGame::load_for_language(config.learn_english);
                }
                controller.config = *config;
                controller.translator = None;
                tracing::info!(
                    enabled = controller.config.trigger.enabled,
                    english_practice = controller.config.english_practice,
                    learn_english = controller.config.learn_english,
                    practice_demo = controller.config.practice_demo,
                    key = ?controller.config.trigger.key,
                    paused = controller.paused,
                    "全局语音配置已接收"
                );
                let configured = controller.hook.configure(TriggerConfig {
                    enabled: controller.config.trigger.enabled && !controller.paused,
                    ..controller.config.trigger
                });
                controller.permitted.configure(
                    revision,
                    controller.config.trigger.enabled && !controller.paused && configured.is_ok(),
                );
                if let Err(message) = configured {
                    tracing::error!(%message, "全局语音按键配置失败");
                }
                if controller.config.trigger.enabled && !controller.paused {
                    controller.voice.prewarm();
                    controller.prewarm_demo();
                }
            }
            Ok(Command::PracticeAction(action)) => controller.practice_action(action),
            Ok(Command::SaveShortcut(shortcut, path, reply, deadline)) => {
                if Instant::now() > deadline {
                    tracing::warn!("跟读快捷键保存请求在语音线程排队过久，未执行");
                    let _ = reply.send(Err("快捷键保存请求已过期".to_owned()));
                    continue;
                }
                let result = controller.hook.save_shortcut(shortcut, path);
                if result.is_ok() {
                    controller.cancel();
                    controller.config.trigger.practice_shortcut = shortcut;
                    let revision = controller.permitted.invalidate();
                    controller.permitted.configure(
                        revision,
                        controller.config.trigger.enabled && !controller.paused,
                    );
                }
                let _ = reply.send(result);
            }
            Err(RecvTimeoutError::Timeout) => {}
        }
        while let Some(event) = controller.hook.try_recv() {
            controller.event(event);
        }
        controller.tick();
    }
    controller.cancel();
}

struct Controller {
    sink: Box<dyn VoiceSink>,

    targets: TargetContext,

    hook: HookThread,

    voice: VoiceManager,

    permitted: Arc<Permission>,

    config: VoiceConfig,

    active: Option<Active>,

    practice: Option<PracticeSession>,

    translator: Option<Translator>,

    demo: Option<EnglishDemo>,

    demo_ready_at: Option<(Instant, bool)>,

    pronunciation_request: Option<u64>,

    pronunciation_sequence: u64,

    game: PracticeGame,

    last_meter_update: Instant,

    paused: bool,
}

impl Controller {
    fn cancel(&mut self) {
        self.pronunciation_request = None;
        self.sink.set_voice_level(0);
        if let Some(active) = &self.active {
            tracing::info!(
                sequence = active.sequence,
                phase = ?self.practice.as_ref().map(|session| session.phase),
                elapsed_ms = active.since.elapsed().as_millis(),
                started = active.started,
                held = active.held,
                activity_source = ?self.hook.activity_source(),
                "全局语音轮次清理"
            );
        }
        self.voice.cancel();
        if let Some(translator) = &mut self.translator {
            translator.cancel();
        }
        if let Some(demo) = &mut self.demo {
            demo.cancel();
        }
        self.demo_ready_at = None;
        self.game.cancel();
        self.practice = None;
        self.hook.set_practice_active(false);
        self.active = None;
        self.sink.set_voice_held(false);
        self.sink.hide_voice();
    }

    fn event(&mut self, event: TriggerEvent) {
        match event {
            TriggerEvent::Press {
                sequence,
                revision,
                mode,
            } => {
                self.cancel();
                if !self.permitted.allowed()
                    || !self.hook.is_held(sequence)
                    || !self.config.accepts_mode(mode)
                {
                    return;
                }
                self.prewarm_demo();
                let captured = self.targets.capture();
                if let Err(error) = &captured {
                    tracing::warn!(sequence, ?error, "全局语音目标捕获失败");
                }
                if !self.permitted.allowed()
                    || self.hook.activity_revision() != revision
                    || !self.hook.is_held(sequence)
                {
                    tracing::info!(
                        sequence,
                        permitted = self.permitted.allowed(),
                        activity_current = self.hook.activity_revision() == revision,
                        held = self.hook.is_held(sequence),
                        activity_source = ?self.hook.activity_source(),
                        "全局语音捕获期间已取消"
                    );
                    return;
                }
                let (target, error) = match captured {
                    Ok(target) => (Some(target), None),
                    Err(error) => (None, Some(error.user_message().to_owned())),
                };
                self.active = Some(Active {
                    sequence,
                    revision,
                    mode,
                    target,
                    error,
                    held: true,
                    started: false,
                    final_text: None,
                    since: Instant::now(),
                    pending_key_event: None,
                });
            }
            TriggerEvent::Held { sequence, revision } => {
                let Some(active) = self
                    .active
                    .as_mut()
                    .filter(|active| active.sequence == sequence)
                else {
                    return;
                };
                if active.revision != revision
                    || !self.permitted.allowed()
                    || self.hook.activity_revision() != revision
                    || !self.hook.is_held(sequence)
                    || !self.hook.healthy()
                {
                    tracing::info!(
                        sequence,
                        same_revision = active.revision == revision,
                        permitted = self.permitted.allowed(),
                        activity_current = self.hook.activity_revision() == revision,
                        held = self.hook.is_held(sequence),
                        healthy = self.hook.healthy(),
                        "全局语音长按启动被拒绝"
                    );
                    self.cancel();
                    return;
                }
                let rect = active
                    .target
                    .as_ref()
                    .map_or_else(empty_rect, |target| target.rect());
                self.sink.set_voice_held(true);
                if let Some(message) = &active.error {
                    tracing::info!(sequence, "全局语音长按启动因目标捕获失败被拒绝");
                    self.sink.voice_error(
                        message.clone(),
                        rect,
                        self.config.recording_mode(active.mode),
                    );
                    return;
                }
                if active.target.as_ref().is_none_or(|target| {
                    match self.targets.validate_focus(target) {
                        Ok(()) => false,
                        Err(error) => {
                            tracing::warn!(sequence, ?error, "全局语音长按目标复核失败");
                            true
                        }
                    }
                }) {
                    self.cancel();
                    return;
                }
                active.started = true;
                active.since = Instant::now();
                active.pending_key_event = None;
                self.sink.set_voice_held(true);
                self.sink
                    .prepare_voice(rect, self.config.recording_mode(active.mode));
                let update = self
                    .voice
                    .start_in_language(self.config.dictation_language(active.mode));
                tracing::info!(sequence, mode = ?active.mode, "全局语音开始");
                self.update(update);
            }
            TriggerEvent::Release { sequence, revision } => {
                let Some(active) = self
                    .active
                    .as_mut()
                    .filter(|active| active.sequence == sequence)
                else {
                    return;
                };
                active.held = false;
                active.revision = revision;
                active.since = Instant::now();
                self.sink.hide_voice();
                if !active.started || active.error.is_some() {
                    self.cancel();
                } else if let Some(text) = active.final_text.take() {
                    self.dictation_finished(text);
                } else {
                    let update = self.voice.stop();
                    tracing::info!(sequence, "全局语音松开，等待尾音结果");
                    self.update(update);
                }
            }
            TriggerEvent::Cancel { sequence, .. } => {
                if self
                    .active
                    .as_ref()
                    .is_some_and(|active| active.sequence == sequence)
                {
                    tracing::info!(sequence, activity_source = ?self.hook.activity_source(), "输入活动通知已取消本轮全局语音");
                    self.cancel();
                }
            }
        }
    }

    fn tick(&mut self) {
        // 30 Hz 与浮窗帧率对齐，减少不必要的 IPC/UI 工作，同时保持音波连贯。
        if self.last_meter_update.elapsed() >= Duration::from_millis(33) {
            self.sink.set_voice_level(self.voice.level());
            self.last_meter_update = Instant::now();
        }
        let Some(active) = self.active.as_mut() else {
            return;
        };
        if self.practice.is_some()
            && self.hook.take_practice_escape()
            && should_close(ExitEvent::Escape)
        {
            self.cancel();
            return;
        }
        let failure = if !self.permitted.allowed() {
            Some("语音配置已关闭或后台正在更新")
        } else if !self.hook.healthy() {
            Some("语音按键监听失去响应")
        } else if self.practice.is_none() && active.finish_timed_out(Instant::now()) {
            Some("松键后的语音收尾超时")
        } else if self.practice.is_none()
            && active
                .target
                .as_ref()
                .is_some_and(|target| self.targets.validate_focus(target).is_err())
        {
            Some("原输入框或输入桌面已变化")
        } else {
            None
        };
        if let Some(reason) = failure {
            tracing::warn!(
                sequence = active.sequence,
                held = active.held,
                elapsed_ms = active.since.elapsed().as_millis(),
                reason,
                "全局语音已取消"
            );
            self.cancel();
            return;
        }
        let key_changed = self.hook.activity_revision() != active.revision
            || (active.held && !self.hook.is_held(active.sequence));
        if key_changed {
            // 钩子先发布原子快照再投递 Release；留一拍接收，不能把正常松键当取消。
            let first_seen = active.pending_key_event.get_or_insert_with(Instant::now);
            if !active.held || first_seen.elapsed() >= TICK {
                tracing::info!(sequence = active.sequence, activity_source = ?self.hook.activity_source(), "输入活动状态变化已取消本轮全局语音");
                self.cancel();
            }
            return;
        }
        active.pending_key_event = None;
        if self.practice.is_some() {
            self.sink.set_voice_held(true);
            self.practice_tick();
            return;
        }
        if active.held && (active.started || active.error.is_some()) {
            self.sink.set_voice_held(true);
        }
        if active.started
            && active.error.is_none()
            && let Some(update) = self.voice.poll()
        {
            self.update(update);
        }
    }

    fn update(&mut self, update: VoiceUpdate) {
        if self.practice.is_some() {
            self.practice_update(update);
            return;
        }
        let Some(active) = self.active.as_mut() else {
            return;
        };
        let rect = active
            .target
            .as_ref()
            .map_or_else(empty_rect, |target| target.rect());
        let recording_mode = self.config.recording_mode(active.mode);
        match update {
            VoiceUpdate::Started if active.held => {
                self.sink.show_voice(String::new(), rect, recording_mode)
            }
            VoiceUpdate::Interim { text } if active.held => {
                self.sink.show_voice(text, rect, recording_mode)
            }
            VoiceUpdate::Final { text } if active.held => {
                self.sink.show_voice(text.clone(), rect, recording_mode);
                active.final_text = Some(text);
            }
            VoiceUpdate::Final { text } => self.dictation_finished(text),
            VoiceUpdate::Error { message } => {
                tracing::warn!(%message, "全局语音识别失败");
                self.voice.cancel();
                if active.held {
                    self.sink.voice_error(message.clone(), rect, recording_mode);
                    active.error = Some(message);
                } else {
                    self.cancel();
                }
            }
            _ => {}
        }
    }

    fn finish(&mut self, text: String) {
        let Some(active) = self.active.as_ref() else {
            return;
        };
        if text.is_empty() {
            if let Some(session) = &mut self.practice {
                session.fail("没有可填入的中文内容；请按 Esc 取消".to_owned());
                self.show_practice();
            } else {
                self.active = None;
                self.game.cancel();
                self.hook.set_practice_active(false);
                self.sink.hide_voice();
                self.sink.set_voice_held(false);
            }
            return;
        }

        let sequence = active.sequence;
        let revision = active.revision;
        let Some(target) = active.target.as_ref() else {
            if let Some(session) = &mut self.practice {
                session.fail("原输入框已不可用，中文尚未填入；请按 Esc 取消".to_owned());
                self.show_practice();
            } else {
                self.active = None;
                self.game.cancel();
                self.sink.hide_voice();
                self.sink.set_voice_held(false);
            }
            return;
        };
        let process = target.process_id();
        let thread = target.thread_id();
        let delivery_method = target.delivery_method();
        tracing::info!(
            sequence,
            process,
            thread,
            delivery_method,
            "准备交付语音文字"
        );
        let permitted = &self.permitted;
        let hook = &self.hook;
        let result = self.targets.insert(target, &text, || {
            permitted.allowed() && hook.healthy() && hook.activity_revision() == revision
        });
        match result {
            Ok(units) => {
                self.active = None;
                let practice_progress = if should_close(ExitEvent::Inserted) {
                    self.practice.take().map(|session| session.progress())
                } else {
                    None
                };
                self.hook.set_practice_active(false);
                self.sink.set_voice_level(0);
                self.sink.hide_voice();
                self.sink.set_voice_held(false);
                self.voice.cancel();
                tracing::info!(
                    sequence,
                    characters = text.chars().count(),
                    "全局语音识别完成"
                );
                if delivery_method == "tsf" {
                    tracing::info!(
                        sequence,
                        process,
                        thread,
                        delivery_method,
                        units,
                        "宿主已确认语音插入"
                    );
                } else {
                    tracing::info!(
                        sequence,
                        process,
                        thread,
                        delivery_method,
                        units,
                        "系统已接收语音输入事件，正文需由宿主处理"
                    );
                }
                // 完整目标复核和系统交付成功之后才领奖，取消的轮次不能留下经验。
                if let Some(progress) = practice_progress {
                    self.game.complete(progress);
                } else {
                    self.game.cancel();
                }
            }
            Err(error) => {
                self.game.cancel();
                if !should_close(ExitEvent::InsertFailed)
                    && let Some(session) = &mut self.practice
                {
                    session.fail(format!(
                        "中文尚未填入：{}。恢复原输入框后可重试，或按 Esc 取消",
                        error.user_message()
                    ));
                    self.show_practice();
                } else {
                    self.active = None;
                    self.sink.set_voice_level(0);
                    self.sink.hide_voice();
                    self.sink.set_voice_held(false);
                    self.voice.cancel();
                }
                tracing::warn!(
                    sequence,
                    process,
                    thread,
                    delivery_method,
                    ?error,
                    message = error.user_message(),
                    "全局语音交付失败"
                );
            }
        }
    }
}

fn empty_rect() -> ScreenRect {
    ScreenRect {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    }
}
