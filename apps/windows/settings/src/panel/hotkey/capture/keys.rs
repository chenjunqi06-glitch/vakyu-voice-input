//! 实体按键录入状态；修饰键组合等全部松开后确认，兼容后续追加字母。

use qingjian_platform::PracticeShortcut;

pub(super) struct Keys {
    pressed: [bool; 256],

    suppressed: [bool; 256],

    modifiers_seen: u8,

    result: Option<Result<PracticeShortcut, String>>,
}

impl Keys {
    pub(super) fn new(pressed: [bool; 256]) -> Self {
        Self {
            modifiers_seen: side_modifiers(&pressed),
            pressed,
            suppressed: [false; 256],
            result: None,
        }
    }

    pub(super) fn key(&mut self, vk: u16, down: bool) -> bool {
        let index = usize::from(vk);
        if index >= self.pressed.len() {
            return false;
        }
        let repeated = self.pressed[index];
        let consumed = self.suppressed[index];
        self.pressed[index] = down;
        if !down {
            self.suppressed[index] = false;
        }
        if self.result.is_some() || down && repeated {
            return consumed;
        }
        if down {
            self.suppressed[index] = true;
            if vk == 0x1b {
                self.result = Some(Err("已取消快捷键录制".to_owned()));
            } else if modifier_bit(vk) != 0 {
                self.modifiers_seen |= modifier_bit(vk);
            } else {
                self.result = Some(PracticeShortcut::from_keys(vk, self.modifiers()));
            }
            return true;
        }
        if modifier_bit(vk) != 0 && side_modifiers(&self.pressed) == 0 {
            self.result = Some(match self.modifiers_seen {
                0b1010 => Ok(PracticeShortcut::RIGHT_CTRL_ALT),
                0b0100 => PracticeShortcut::from_keys(0xa4, 0),
                0b1000 => PracticeShortcut::from_keys(0xa5, 0),
                _ => Err(
                    "这组修饰键不能单独使用；可按右 Ctrl + 右 Alt，或加上字母/功能键。".to_owned(),
                ),
            });
        }
        consumed
    }

    pub(super) fn take_result(&mut self) -> Option<Result<PracticeShortcut, String>> {
        // 成对吞完录制期间按下的键，避免残留 key-up 触发原快捷键或后台录音。
        if self.suppressed.iter().any(|pressed| *pressed) {
            None
        } else {
            self.result.take()
        }
    }

    fn modifiers(&self) -> u8 {
        let any = |left: usize, right: usize| self.pressed[left] || self.pressed[right];
        u8::from(any(0xa4, 0xa5))
            | (u8::from(any(0xa2, 0xa3)) << 1)
            | (u8::from(any(0xa0, 0xa1)) << 2)
            | (u8::from(any(0x5b, 0x5c)) << 3)
    }
}

fn modifier_bit(vk: u16) -> u8 {
    match vk {
        0xa2 => 1,
        0xa3 => 2,
        0xa4 => 4,
        0xa5 => 8,
        0xa0 => 16,
        0xa1 => 32,
        0x5b => 64,
        0x5c => 128,
        _ => 0,
    }
}

fn side_modifiers(pressed: &[bool; 256]) -> u8 {
    [0xa2, 0xa3, 0xa4, 0xa5, 0xa0, 0xa1, 0x5b, 0x5c]
        .into_iter()
        .filter(|vk| pressed[usize::from(*vk)])
        .fold(0, |bits, vk| bits | modifier_bit(vk))
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
