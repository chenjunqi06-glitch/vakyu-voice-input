//! 普通口述与跟读共用的按住状态机；短按与组合键按原始扫描码恢复。

use super::{
    active::Active, config::TriggerConfig, event::TriggerEvent, input::InputKey, mode::VoiceMode,
    outcome::Outcome,
};
use std::time::{Duration, Instant};

pub(super) struct Machine {
    config: TriggerConfig,

    pressed: [bool; 256],

    next_sequence: u64,

    released_chord_key: Option<u16>,

    releasing_shortcut: Option<qingjian_platform::PracticeShortcut>,

    /// 右 Alt 先按时先暂扣，避免后续 Ctrl 被全局热键截获后宿主只看见单独 Alt。
    pending_alt: Option<InputKey>,

    /// 双键成立后，先暂扣的 Alt 也必须成对吞掉抬起。
    suppressed_alt: bool,

    pub active: Option<Active>,
}

impl Machine {
    pub fn configuration(&self) -> TriggerConfig {
        self.config
    }

    pub fn new(config: TriggerConfig, pressed: [bool; 256]) -> Self {
        Self {
            config,
            pressed,
            next_sequence: 0,
            released_chord_key: None,
            releasing_shortcut: None,
            pending_alt: None,
            suppressed_alt: false,
            active: None,
        }
    }

    pub fn configure(&mut self, config: TriggerConfig, revision: u64) -> Outcome {
        if self.config == config {
            return Outcome::default();
        }
        let outcome = self.cancel(revision);
        self.config = config;
        outcome
    }

    pub fn key(&mut self, key: InputKey, revision: u64, now: Instant) -> Outcome {
        let index = usize::from(key.vk);
        if index >= self.pressed.len() {
            return Outcome::default();
        }
        let was_pressed = self.pressed[index];
        self.pressed[index] = key.down;
        let suppressed_alt = self.suppressed_alt && key.vk == 0xa5;
        if suppressed_alt && !key.down {
            self.suppressed_alt = false;
        }
        if self.releasing_shortcut.is_some_and(|shortcut| {
            !self.pressed[usize::from(shortcut.key)]
                && pressed_modifiers(&self.pressed, shortcut.key) & shortcut.modifiers == 0
        }) {
            self.releasing_shortcut = None;
        }
        if self.released_chord_key == Some(key.vk) {
            if !key.down {
                self.released_chord_key = None;
            }
            return Outcome {
                consume: true,
                ..Outcome::default()
            };
        }
        if let Some(pending) = self.pending_alt {
            if key.vk == pending.vk {
                if key.down {
                    return Outcome {
                        consume: true,
                        ..Outcome::default()
                    };
                }
                self.pending_alt = None;
                return Outcome {
                    consume: true,
                    replay_alt: Some(pending),
                    replay_release: true,
                    ..Outcome::default()
                };
            }
            self.pending_alt = None;
            if key.down
                && key.vk == 0xa3
                && !self
                    .pressed
                    .iter()
                    .enumerate()
                    .any(|(vk, down)| *down && !matches!(vk, 0x10..=0x12 | 0xa3 | 0xa5))
            {
                self.suppressed_alt = true;
            } else {
                return Outcome {
                    consume: true,
                    replay_alt: Some(pending),
                    ..Outcome::default()
                };
            }
        }
        if suppressed_alt && self.active.is_none() {
            return Outcome {
                consume: true,
                ..Outcome::default()
            };
        }
        if let Some(active) = &self.active {
            if key.vk == active.key.vk {
                if key.down {
                    return Outcome {
                        consume: true,
                        ..Outcome::default()
                    };
                }
                let active = self.active.take().expect("已检查活动轮次");
                if active.modifiers() != 0 {
                    self.releasing_shortcut = active.shortcut;
                }
                return Outcome {
                    consume: true,
                    replay_alt: (!active.held && active.modifiers() == 0).then_some(active.key),
                    replay_release: !active.held && active.modifiers() == 0,
                    event: Some(TriggerEvent::Release {
                        sequence: active.sequence,
                        revision,
                    }),
                };
            }
            if active
                .shortcut
                .is_some_and(|shortcut| shortcut.is_modifier_key(key.vk))
            {
                if key.down {
                    return Outcome {
                        consume: suppressed_alt,
                        ..Outcome::default()
                    };
                }
                let active = self.active.take().expect("已检查活动轮次");
                self.released_chord_key = Some(active.key.vk);
                self.releasing_shortcut = active.shortcut;
                return Outcome {
                    consume: suppressed_alt,
                    event: Some(TriggerEvent::Release {
                        sequence: active.sequence,
                        revision,
                    }),
                    ..Outcome::default()
                };
            }
            let mut outcome = self.cancel(revision);
            outcome.consume = outcome.replay_alt.is_some();
            return outcome;
        }
        let modifiers = pressed_modifiers(&self.pressed, key.vk);
        // 纯修饰双键允许任意按下顺序；实体右 Ctrl 与右 Alt 必须同时按住。
        // AltGr 合成的左 Ctrl 和其他附加键仍由 others_pressed 排除。
        let modifiers_match =
            if self.config.practice_enabled && self.config.practice_shortcut.is_right_ctrl_alt() {
                self.pressed[0xa3] && self.pressed[0xa5]
            } else {
                modifiers == self.config.practice_shortcut.modifiers
            };
        let others_pressed = self.pressed.iter().enumerate().any(|(vk, down)| {
            *down
                && vk != index
                && !matches!(vk, 0x10..=0x12)
                && !(self.config.practice_enabled
                    && self.config.practice_shortcut.is_modifier_key(vk as u16))
        });
        if key.down
            && !was_pressed
            && !others_pressed
            && modifiers == 0
            && self.config.enabled
            && self.config.practice_enabled
            && self.config.practice_shortcut.is_right_ctrl_alt()
            && key.vk == 0xa5
        {
            self.pending_alt = Some(key);
            return Outcome {
                consume: true,
                ..Outcome::default()
            };
        }
        let mode = if !others_pressed && modifiers_match && self.config.accepts_practice(key.vk) {
            Some(VoiceMode::Practice)
        } else if modifiers == 0
            && self.config.accepts_dictation(key.vk)
            && !self
                .pressed
                .iter()
                .enumerate()
                .any(|(vk, down)| *down && vk != index && !matches!(vk, 0x10..=0x12))
        {
            Some(VoiceMode::Dictation)
        } else {
            None
        };
        if let Some(mode) = mode.filter(|_| key.down && !was_pressed) {
            self.next_sequence = self.next_sequence.wrapping_add(1).max(1);
            self.active = Some(Active {
                key,
                sequence: self.next_sequence,
                pressed: now,
                held: false,
                mode,
                shortcut: (mode == VoiceMode::Practice).then_some(self.config.practice_shortcut),
            });
            return Outcome {
                consume: true,
                event: Some(TriggerEvent::Press {
                    sequence: self.next_sequence,
                    revision,
                    mode,
                }),
                ..Outcome::default()
            };
        }
        Outcome::default()
    }

    pub fn tick(&mut self, revision: u64, now: Instant) -> Outcome {
        let Some(active) = self.active.as_mut() else {
            return Outcome::default();
        };
        if !active.held
            && now.saturating_duration_since(active.pressed)
                >= Duration::from_millis(u64::from(self.config.hold_ms.clamp(100, 1000)))
        {
            active.held = true;
            return Outcome {
                event: Some(TriggerEvent::Held {
                    sequence: active.sequence,
                    revision,
                }),
                ..Outcome::default()
            };
        }
        Outcome::default()
    }

    /// 组合键松开会产生多条 key-up，它们属于同一次结束，不能把等待尾音当成新输入取消。
    pub fn changes_activity(&self, key: InputKey) -> bool {
        if key.down
            && self
                .pressed
                .get(usize::from(key.vk))
                .copied()
                .unwrap_or(false)
        {
            return false;
        }
        !(!key.down
            && self
                .releasing_shortcut
                .is_some_and(|shortcut| shortcut.key == key.vk || shortcut.is_modifier_key(key.vk)))
    }

    pub fn observe_key(&mut self, key: InputKey) -> bool {
        let suppressed_alt = self.suppressed_alt && key.vk == 0xa5;
        let suppressed_chord = self.released_chord_key == Some(key.vk);
        if suppressed_alt && !key.down {
            self.suppressed_alt = false;
        }
        if let Some(pressed) = self.pressed.get_mut(usize::from(key.vk)) {
            *pressed = key.down;
        }
        if !key.down && self.released_chord_key == Some(key.vk) {
            self.released_chord_key = None;
        }
        if self.releasing_shortcut.is_some_and(|shortcut| {
            !self.pressed[usize::from(shortcut.key)]
                && pressed_modifiers(&self.pressed, shortcut.key) & shortcut.modifiers == 0
        }) {
            self.releasing_shortcut = None;
        }
        suppressed_alt || suppressed_chord
    }

    pub fn injected_key(&mut self, key: InputKey, revision: u64) -> Outcome {
        if let Some(pressed) = self.pressed.get_mut(usize::from(key.vk)) {
            *pressed = key.down;
        }
        self.cancel(revision)
    }

    pub fn cancel(&mut self, revision: u64) -> Outcome {
        if let Some(pending) = self.pending_alt.take() {
            return Outcome {
                replay_alt: Some(pending),
                ..Outcome::default()
            };
        }
        let Some(active) = self.active.take() else {
            return Outcome::default();
        };
        if active.modifiers() != 0 {
            self.released_chord_key = Some(active.key.vk);
        }
        Outcome {
            replay_alt: (active.modifiers() == 0).then_some(active.key),
            event: Some(TriggerEvent::Cancel {
                sequence: active.sequence,
                revision,
            }),
            ..Outcome::default()
        }
    }

    pub fn mouse(&mut self, button: Option<(u16, bool)>, revision: u64) -> Outcome {
        if let Some((vk, down)) = button {
            self.pressed[usize::from(vk)] = down;
        }
        let mut outcome = self.cancel(revision);
        outcome.consume = outcome.replay_alt.is_some();
        outcome
    }
}

fn pressed_modifiers(pressed: &[bool; 256], main_key: u16) -> u8 {
    let mut modifiers = 0;
    for (bit, keys) in [
        (1, [0xa4, 0xa5]),
        (2, [0xa2, 0xa3]),
        (4, [0xa0, 0xa1]),
        (8, [0x5b, 0x5c]),
    ] {
        if keys
            .into_iter()
            .any(|key| key != main_key && pressed[usize::from(key)])
        {
            modifiers |= bit;
        }
    }
    modifiers
}
