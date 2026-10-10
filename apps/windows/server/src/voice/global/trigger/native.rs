//! Win32 低层钩子与消息泵；回调只处理单个输入、更新原子状态并投递小事件。

use super::{
    activity::{ActivitySource, ignored_while_practicing, is_practice_escape},
    command::Command,
    config::TriggerConfig,
    event::TriggerEvent,
    guard::HookGuard,
    input::InputKey,
    machine::Machine,
    outcome::Outcome,
    registration::Registration,
    runtime::Runtime,
    shared::Shared,
};
use crate::voice::global::target::GLOBAL_VOICE_INPUT_TAG;
use std::cell::RefCell;
use std::sync::{
    Arc,
    atomic::Ordering,
    mpsc::{Receiver, Sender, SyncSender},
};
use std::time::Instant;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::{LibraryLoader::GetModuleHandleW, Threading::GetCurrentThreadId};
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBD_EVENT_FLAGS, KEYBDINPUT,
    KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE, MOUSEEVENTF_HWHEEL,
    MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP,
    MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_WHEEL, MOUSEEVENTF_XDOWN,
    MOUSEEVENTF_XUP, MOUSEINPUT, SendInput, VIRTUAL_KEY,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, EVENT_OBJECT_FOCUS, EVENT_SYSTEM_FOREGROUND, GA_ROOT,
    GetAncestor, GetForegroundWindow, GetMessageW, KBDLLHOOKSTRUCT, LLKHF_EXTENDED, LLKHF_INJECTED,
    LLMHF_INJECTED, MSG, MSLLHOOKSTRUCT, PM_NOREMOVE, PeekMessageW, SetTimer, SetWindowsHookExW,
    TranslateMessage, WH_KEYBOARD_LL, WH_MOUSE_LL, WINEVENT_OUTOFCONTEXT, WM_APP, WM_KEYDOWN,
    WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEHWHEEL,
    WM_MOUSEWHEEL, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SYSKEYDOWN, WM_SYSKEYUP, WM_TIMER,
    WM_XBUTTONDOWN, WM_XBUTTONUP,
};

pub(super) const CONFIG_MESSAGE: u32 = WM_APP + 0x52;

thread_local! {
    static RUNTIME: RefCell<Option<Runtime>> = const { RefCell::new(None) };
}

pub(super) fn run(
    config: TriggerConfig,
    commands: Receiver<Command>,
    events: Sender<TriggerEvent>,
    shared: Arc<Shared>,
    ready: SyncSender<Result<u32, String>>,
) {
    let result = run_inner(config, commands, events, Arc::clone(&shared), &ready);
    shared.alive.store(false, Ordering::Release);
    shared.sequence.store(0, Ordering::Release);
    if let Err(error) = result {
        let _ = ready.send(Err(error));
    }
}

fn run_inner(
    config: TriggerConfig,
    commands: Receiver<Command>,
    events: Sender<TriggerEvent>,
    shared: Arc<Shared>,
    ready: &SyncSender<Result<u32, String>>,
) -> Result<(), String> {
    let mut message = MSG::default();
    unsafe {
        let _ = PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
    }
    let mut desired_config = config;
    let (mut registration, initial_config) = match Registration::new(
        config.practice_shortcut,
        config.enabled && config.practice_enabled,
    ) {
        Ok(registration) => (registration, config),
        Err(message) => {
            tracing::warn!(%message, "跟读快捷键已被占用，保留后台以便在设置中更换");
            (
                Registration::new(config.practice_shortcut, false)?,
                TriggerConfig {
                    practice_enabled: false,
                    ..config
                },
            )
        }
    };
    let mut pressed = [false; 256];
    for (vk, value) in pressed.iter_mut().enumerate() {
        if !(0x10..=0x12).contains(&vk) {
            *value = unsafe { GetAsyncKeyState(vk as i32) < 0 };
        }
    }
    RUNTIME.with(|state| {
        *state.borrow_mut() = Some(Runtime {
            machine: Machine::new(initial_config, pressed),
            shared: Arc::clone(&shared),
            events,
        })
    });
    let module =
        unsafe { GetModuleHandleW(None) }.map_err(|error| format!("无法取得按键模块：{error}"))?;
    let mut hooks = HookGuard {
        keyboard: Some(
            unsafe {
                SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook), Some(module.into()), 0)
            }
            .map_err(|error| format!("无法监听全局语音按键：{error}"))?,
        ),
        ..HookGuard::default()
    };
    hooks.mouse = Some(
        unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook), Some(module.into()), 0) }
            .map_err(|error| format!("无法监听语音取消操作：{error}"))?,
    );
    for (event, slot) in [
        (EVENT_SYSTEM_FOREGROUND, &mut hooks.foreground),
        (EVENT_OBJECT_FOCUS, &mut hooks.focus),
    ] {
        let hook = unsafe {
            SetWinEventHook(
                event,
                event,
                None,
                Some(focus_hook),
                0,
                0,
                WINEVENT_OUTOFCONTEXT,
            )
        };
        if hook.0.is_null() {
            return Err("无法监听语音输入焦点变化".to_owned());
        }
        *slot = Some(hook);
    }
    hooks.timer = unsafe { SetTimer(None, 0, 15, None) };
    if hooks.timer == 0 {
        return Err("无法创建全局语音按键计时器".to_owned());
    }
    let _ = ready.send(Ok(unsafe { GetCurrentThreadId() }));
    while unsafe { GetMessageW(&mut message, None, 0, 0) }.0 > 0 {
        shared
            .heartbeat
            .store(shared.elapsed_ms(), Ordering::Release);
        if message.message == CONFIG_MESSAGE || message.message == WM_TIMER {
            for command in commands.try_iter() {
                let (config, reply, result, apply_config) = match command {
                    Command::Configure(config, reply) => {
                        desired_config = config;
                        let mut result = registration.replace(
                            config.practice_shortcut,
                            config.enabled && config.practice_enabled,
                            None,
                        );
                        let effective = if result.is_ok() {
                            config
                        } else {
                            // 跟读抢占失败不能连带关闭普通语音；释放旧跟读占位以便用户更换。
                            if let Err(message) = &result {
                                tracing::warn!(%message, "跟读快捷键已被占用，普通语音输入继续可用");
                            }
                            let _ = registration.replace(config.practice_shortcut, false, None);
                            // configure 的确认表示钩子已应用可工作的配置；跟读降级不能让
                            // 上层把普通入口也标记为不可用，真实通信失败仍由 HookThread 返回。
                            result = Ok(());
                            TriggerConfig {
                                practice_enabled: false,
                                ..config
                            }
                        };
                        (effective, reply, result, true)
                    }
                    Command::SaveShortcut(shortcut, path, reply) => {
                        let config = desired_config;
                        let result = if config.key.conflicts_with_practice(shortcut) {
                            Err("该快捷键与普通语音转文字按键冲突，请更换其中一个按键".to_owned())
                        } else {
                            registration.replace(
                                shortcut,
                                config.enabled && config.practice_enabled,
                                Some(&path),
                            )
                        };
                        if result.is_ok() {
                            desired_config.practice_shortcut = shortcut;
                        }
                        let apply_config = result.is_ok();
                        (
                            TriggerConfig {
                                practice_shortcut: shortcut,
                                ..config
                            },
                            reply,
                            result,
                            apply_config,
                        )
                    }
                };
                if apply_config {
                    let outcome = with_runtime(|runtime| {
                        if runtime.machine.configuration() == config {
                            Outcome::default()
                        } else {
                            runtime.machine.configure(
                                config,
                                runtime.shared.activity(ActivitySource::Configuration),
                            )
                        }
                    });
                    replay(&outcome, None);
                }
                let _ = reply.send(result);
            }
            let outcome = with_runtime(|runtime| {
                runtime.machine.tick(
                    runtime.shared.revision.load(Ordering::Acquire),
                    Instant::now(),
                )
            });
            replay(&outcome, None);
        } else {
            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
    let outcome = with_runtime(|runtime| {
        runtime
            .machine
            .cancel(runtime.shared.activity(ActivitySource::Shutdown))
    });
    replay(&outcome, None);
    RUNTIME.with(|state| *state.borrow_mut() = None);
    Ok(())
}

fn with_runtime(action: impl FnOnce(&mut Runtime) -> Outcome) -> Outcome {
    RUNTIME.with(|state| {
        let mut state = state.borrow_mut();
        let Some(runtime) = state.as_mut() else {
            return Outcome::default();
        };
        let outcome = action(runtime);
        runtime.publish(&outcome);
        outcome
    })
}

fn cancel_activity(source: ActivitySource) {
    let outcome = with_runtime(|runtime| runtime.machine.cancel(runtime.shared.activity(source)));
    replay(&outcome, None);
}

unsafe extern "system" fn focus_hook(
    _hook: HWINEVENTHOOK,
    event: u32,
    window: HWND,
    _object: i32,
    _child: i32,
    _thread: u32,
    _time: u32,
) {
    if practice_active() && ignored_while_practicing(ActivitySource::FocusNotification) {
        return;
    }
    if event == EVENT_OBJECT_FOCUS
        && !window.0.is_null()
        && unsafe { GetAncestor(window, GA_ROOT) != GetForegroundWindow() }
    {
        return;
    }
    // 只保留活动代次和类别，不采集窗口标题、控件内容或焦点对象。
    cancel_activity(if event == EVENT_SYSTEM_FOREGROUND {
        ActivitySource::Foreground
    } else {
        ActivitySource::FocusNotification
    });
}

unsafe extern "system" fn keyboard_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code < 0 {
        return unsafe { CallNextHookEx(None, code, wparam, lparam) };
    }
    let raw = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
    let message = wparam.0 as u32;
    if !matches!(message, WM_KEYDOWN | WM_KEYUP | WM_SYSKEYDOWN | WM_SYSKEYUP) {
        return unsafe { CallNextHookEx(None, code, wparam, lparam) };
    }
    let extended = raw.flags.0 & LLKHF_EXTENDED.0 != 0;
    let vk = match raw.vkCode {
        0x10 => {
            if raw.scanCode == 0x36 {
                0xa1
            } else {
                0xa0
            }
        }
        0x11 => {
            if extended {
                0xa3
            } else {
                0xa2
            }
        }
        0x12 => {
            if extended {
                0xa5
            } else {
                0xa4
            }
        }
        key => key as u16,
    };
    let key = InputKey {
        vk,
        scan: raw.scanCode as u16,
        extended,
        down: matches!(message, WM_KEYDOWN | WM_SYSKEYDOWN),
    };
    let practicing = practice_active();
    if is_practice_escape(practicing, raw.flags.0 & LLKHF_INJECTED.0 == 0, key.vk) {
        if key.down {
            RUNTIME.with(|state| {
                if let Some(runtime) = state.borrow().as_ref() {
                    runtime
                        .shared
                        .practice_escape
                        .store(true, Ordering::Release);
                }
            });
        }
        return LRESULT(1);
    }
    if practicing && ignored_while_practicing(ActivitySource::PhysicalKeyboard) {
        if raw.flags.0 & LLKHF_INJECTED.0 == 0 {
            let consumed = RUNTIME.with(|state| {
                if let Some(runtime) = state.borrow_mut().as_mut() {
                    return runtime.machine.observe_key(key);
                }
                false
            });
            if consumed {
                return LRESULT(1);
            }
        }
        return unsafe { CallNextHookEx(None, code, wparam, lparam) };
    }
    if raw.flags.0 & LLKHF_INJECTED.0 != 0 {
        if raw.dwExtraInfo != GLOBAL_VOICE_INPUT_TAG {
            let outcome = with_runtime(|runtime| {
                runtime.machine.injected_key(
                    key,
                    runtime.shared.activity(ActivitySource::ForeignInjection),
                )
            });
            replay(&outcome, None);
        }
        return unsafe { CallNextHookEx(None, code, wparam, lparam) };
    }
    let outcome = with_runtime(|runtime| {
        let revision = if !runtime.machine.changes_activity(key) {
            runtime.shared.revision.load(Ordering::Acquire)
        } else {
            runtime.shared.activity(ActivitySource::PhysicalKeyboard)
        };
        runtime.machine.key(key, revision, Instant::now())
    });
    let current =
        (outcome.replay_alt.is_some() && !outcome.replay_release).then(|| keyboard_input(key));
    let replayed = replay(&outcome, current);
    if outcome.consume && replayed {
        LRESULT(1)
    } else {
        unsafe { CallNextHookEx(None, code, wparam, lparam) }
    }
}

unsafe extern "system" fn mouse_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code < 0 {
        return unsafe { CallNextHookEx(None, code, wparam, lparam) };
    }
    if practice_active() && ignored_while_practicing(ActivitySource::PhysicalMouse) {
        return unsafe { CallNextHookEx(None, code, wparam, lparam) };
    }
    let raw = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
    if raw.flags & LLMHF_INJECTED != 0 {
        if raw.dwExtraInfo != GLOBAL_VOICE_INPUT_TAG
            && mouse_input(wparam.0 as u32, raw.mouseData).is_some()
        {
            cancel_activity(ActivitySource::ForeignInjection);
        }
        return unsafe { CallNextHookEx(None, code, wparam, lparam) };
    }
    let Some((current, button)) = mouse_input(wparam.0 as u32, raw.mouseData) else {
        return unsafe { CallNextHookEx(None, code, wparam, lparam) };
    };
    let outcome = with_runtime(|runtime| {
        runtime.machine.mouse(
            button,
            runtime.shared.activity(ActivitySource::PhysicalMouse),
        )
    });
    let replayed = replay(&outcome, outcome.consume.then_some(current));
    if outcome.consume && replayed {
        LRESULT(1)
    } else {
        unsafe { CallNextHookEx(None, code, wparam, lparam) }
    }
}

fn practice_active() -> bool {
    RUNTIME.with(|state| {
        state
            .borrow()
            .as_ref()
            .is_some_and(|runtime| runtime.shared.practice_active.load(Ordering::Acquire))
    })
}

fn replay(outcome: &Outcome, current: Option<INPUT>) -> bool {
    let Some(alt) = outcome.replay_alt else {
        return true;
    };
    let mut inputs = vec![keyboard_input(alt)];
    if outcome.replay_release {
        inputs.push(keyboard_input(alt.release()));
    }
    if let Some(current) = current {
        inputs.push(current);
    }
    let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    if sent as usize != inputs.len() {
        tracing::warn!(sent, expected = inputs.len(), "无法完整恢复 Alt 组合键");
        return false;
    }
    true
}

fn keyboard_input(key: InputKey) -> INPUT {
    let mut flags = KEYBD_EVENT_FLAGS(0);
    if key.scan != 0 {
        flags |= KEYEVENTF_SCANCODE;
    }
    if key.extended {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    if !key.down {
        flags |= KEYEVENTF_KEYUP;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(if key.scan == 0 { key.vk } else { 0 }),
                wScan: key.scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: GLOBAL_VOICE_INPUT_TAG,
            },
        },
    }
}

fn mouse_input(message: u32, data: u32) -> Option<(INPUT, Option<(u16, bool)>)> {
    let (flags, button, mouse_data) = match message {
        WM_LBUTTONDOWN => (MOUSEEVENTF_LEFTDOWN, Some((1, true)), 0),
        WM_LBUTTONUP => (MOUSEEVENTF_LEFTUP, Some((1, false)), 0),
        WM_RBUTTONDOWN => (MOUSEEVENTF_RIGHTDOWN, Some((2, true)), 0),
        WM_RBUTTONUP => (MOUSEEVENTF_RIGHTUP, Some((2, false)), 0),
        WM_MBUTTONDOWN => (MOUSEEVENTF_MIDDLEDOWN, Some((4, true)), 0),
        WM_MBUTTONUP => (MOUSEEVENTF_MIDDLEUP, Some((4, false)), 0),
        WM_XBUTTONDOWN => (
            MOUSEEVENTF_XDOWN,
            Some((if data >> 16 == 1 { 5 } else { 6 }, true)),
            data >> 16,
        ),
        WM_XBUTTONUP => (
            MOUSEEVENTF_XUP,
            Some((if data >> 16 == 1 { 5 } else { 6 }, false)),
            data >> 16,
        ),
        WM_MOUSEWHEEL => (MOUSEEVENTF_WHEEL, None, ((data >> 16) as i16 as i32) as u32),
        WM_MOUSEHWHEEL => (
            MOUSEEVENTF_HWHEEL,
            None,
            ((data >> 16) as i16 as i32) as u32,
        ),
        _ => return None,
    };
    Some((
        INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: 0,
                    dy: 0,
                    mouseData: mouse_data,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: GLOBAL_VOICE_INPUT_TAG,
                },
            },
        },
        button,
    ))
}
