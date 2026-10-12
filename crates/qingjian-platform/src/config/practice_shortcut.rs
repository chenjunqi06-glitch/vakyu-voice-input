//! 英语跟读快捷键：可持久化的修饰键与主键，禁止占用普通单字输入。

use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct PracticeShortcut {
    /// Alt / Ctrl / Shift / Win 分别占 1 / 2 / 4 / 8 位，与注册接口保持一致。
    pub modifiers: u8,

    /// 必须使用右侧的修饰键位；用于右 Ctrl / Command + 右 Alt。
    pub right_modifiers: u8,

    /// Windows 虚拟键；这里只保存数据，不依赖系统 API。
    pub key: u16,
}

impl PracticeShortcut {
    pub const DEFAULT: Self = Self {
        modifiers: 0,
        right_modifiers: 0,
        key: 0xa5,
    };

    pub const RIGHT_CTRL_ALT: Self = Self {
        modifiers: 2,
        right_modifiers: 2,
        key: 0xa5,
    };

    pub const RIGHT_WIN_ALT: Self = Self {
        modifiers: 8,
        right_modifiers: 8,
        key: 0xa5,
    };

    pub fn is_right_ctrl_alt(self) -> bool {
        self == Self::RIGHT_CTRL_ALT
    }

    pub fn from_keys(key: u16, modifiers: u8) -> Result<Self, String> {
        if modifiers & !15 != 0 {
            return Err("快捷键包含不支持的修饰键".to_owned());
        }
        let function = (0x70..=0x87).contains(&key);
        let ordinary = (0x30..=0x39).contains(&key) || (0x41..=0x5a).contains(&key);
        let side_alt = matches!(key, 0xa4 | 0xa5);
        if key == 0x7b {
            return Err("F12 是 Windows 调试保留键，请换一个快捷键".to_owned());
        }
        if !(function || ordinary || side_alt || key == 0x20) {
            return Err("请选择字母组合键、F 功能键或单独的左 / 右 Alt".to_owned());
        }
        if side_alt && modifiers != 0 {
            return Err("修饰键组合还需要一个字母或功能键".to_owned());
        }
        if (ordinary || key == 0x20) && modifiers & (1 | 2 | 8) == 0 {
            return Err(
                "单独字母、数字、空格及 Shift+字符会影响正常输入，请加入 Ctrl、Alt 或 Win"
                    .to_owned(),
            );
        }
        if key == 0x73 && modifiers == 1 {
            return Err("Alt+F4 用于关闭窗口，请换一个快捷键".to_owned());
        }
        Ok(Self {
            modifiers,
            right_modifiers: 0,
            key,
        })
    }

    pub fn is_modifier_key(self, key: u16) -> bool {
        if self.is_right_ctrl_alt() {
            return matches!(key, 0xa3 | 0xa5);
        }
        if self == Self::RIGHT_WIN_ALT {
            return matches!(key, 0x5c | 0xa5);
        }
        (self.modifiers & 1 != 0 && matches!(key, 0x12 | 0xa4 | 0xa5))
            || (self.modifiers & 2 != 0 && matches!(key, 0x11 | 0xa2 | 0xa3))
            || (self.modifiers & 4 != 0 && matches!(key, 0x10 | 0xa0 | 0xa1))
            || (self.modifiers & 8 != 0 && matches!(key, 0x5b | 0x5c))
    }

    pub fn key_string(self) -> String {
        if self.is_right_ctrl_alt() {
            return "right_ctrl+right_alt".to_owned();
        }
        if self == Self::RIGHT_WIN_ALT {
            return "right_win+right_alt".to_owned();
        }
        let mut parts = Vec::new();
        for (bit, name) in [(2, "ctrl"), (4, "shift"), (1, "alt"), (8, "win")] {
            if self.modifiers & bit != 0 {
                parts.push(name.to_owned());
            }
        }
        parts.push(match self.key {
            0xa4 => "left_alt".to_owned(),
            0xa5 => "right_alt".to_owned(),
            0x20 => "space".to_owned(),
            0x70..=0x87 => format!("f{}", self.key - 0x6f),
            key => char::from_u32(u32::from(key))
                .unwrap_or('?')
                .to_ascii_lowercase()
                .to_string(),
        });
        parts.join("+")
    }

    pub fn label(self) -> String {
        self.key_string()
            .split('+')
            .map(|part| match part {
                "ctrl" => "Ctrl".to_owned(),
                "alt" => "Alt".to_owned(),
                "shift" => "Shift".to_owned(),
                "win" => "Win".to_owned(),
                "right_alt" => "右 Alt".to_owned(),
                "right_ctrl" => "右 Ctrl".to_owned(),
                "right_win" => "右 Win".to_owned(),
                "left_alt" => "左 Alt".to_owned(),
                "space" => "空格".to_owned(),
                key => key.to_ascii_uppercase(),
            })
            .collect::<Vec<_>>()
            .join(" + ")
    }
}

impl Default for PracticeShortcut {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl FromStr for PracticeShortcut {
    type Err = String;
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let normalized = text.trim().to_ascii_lowercase();
        let mut parts = normalized.split('+').map(str::trim).collect::<Vec<_>>();
        if matches!(
            parts.as_slice(),
            ["right_ctrl", "right_alt"] | ["right_alt", "right_ctrl"]
        ) {
            return Ok(Self::RIGHT_CTRL_ALT);
        }
        if matches!(
            parts.as_slice(),
            ["right_win" | "right_cmd" | "right_command", "right_alt"]
                | ["right_alt", "right_win" | "right_cmd" | "right_command"]
        ) {
            return Ok(Self::RIGHT_WIN_ALT);
        }
        let key = parts.pop().ok_or_else(|| "快捷键为空".to_owned())?;
        let mut modifiers = 0;
        for modifier in parts {
            modifiers |= match modifier {
                "ctrl" | "control" => 2,
                "alt" | "option" => 1,
                "shift" => 4,
                "win" | "command" | "cmd" => 8,
                _ => return Err(format!("无法识别修饰键 {modifier}")),
            };
        }
        let key = match key {
            "left_alt" => 0xa4,
            "right_alt" => 0xa5,
            "space" => 0x20,
            key if key.starts_with('f') && key.len() > 1 => key[1..]
                .parse::<u16>()
                .ok()
                .filter(|value| (1..=24).contains(value))
                .map(|value| 0x6f + value)
                .ok_or_else(|| "功能键必须为 F1 至 F24".to_owned())?,
            key if key.len() == 1 => u16::from(key.as_bytes()[0].to_ascii_uppercase()),
            _ => return Err("无法识别快捷键主键".to_owned()),
        };
        Self::from_keys(key, modifiers)
    }
}

impl TryFrom<String> for PracticeShortcut {
    type Error = String;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
impl From<PracticeShortcut> for String {
    fn from(value: PracticeShortcut) -> Self {
        value.key_string()
    }
}
impl fmt::Display for PracticeShortcut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.key_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_combinations_and_preserves_side_alt() {
        let key: PracticeShortcut = " ALT + Control + e ".parse().unwrap();
        assert_eq!(key.key_string(), "ctrl+alt+e");
        assert_eq!(key.label(), "Ctrl + Alt + E");
        assert_eq!(
            "right_alt".parse::<PracticeShortcut>().unwrap(),
            PracticeShortcut::DEFAULT
        );
        for key in ["f8", "left_alt", "ctrl+shift+f9", "win+e", "ctrl+space"] {
            assert_eq!(key.parse::<PracticeShortcut>().unwrap().key_string(), key);
        }
    }

    #[test]
    fn rejects_normal_typing_and_reserved_combinations() {
        for key in [
            "a",
            "9",
            "space",
            "shift+a",
            "ctrl",
            "ctrl+alt",
            "alt+f4",
            "f12",
            "f25",
            "ctrl+enter",
        ] {
            assert!(key.parse::<PracticeShortcut>().is_err(), "{key}");
        }
    }

    #[test]
    fn right_control_alt_round_trips_without_broadening_generic_modifier_chords() {
        for input in ["right_ctrl+right_alt", " RIGHT_ALT + RIGHT_CTRL "] {
            let shortcut: PracticeShortcut = input.parse().unwrap();
            assert_eq!(shortcut, PracticeShortcut::RIGHT_CTRL_ALT);
            assert_eq!(shortcut.key_string(), "right_ctrl+right_alt");
            assert_eq!(shortcut.label(), "右 Ctrl + 右 Alt");
            assert!(shortcut.is_modifier_key(0xa3));
            assert!(shortcut.is_modifier_key(0xa5));
            assert!(!shortcut.is_modifier_key(0xa2));
            assert!(!shortcut.is_modifier_key(0xa4));
        }
        assert!(PracticeShortcut::from_keys(0xa5, 2).is_err());
        for input in ["ctrl+alt", "ctrl+right_alt", "right_ctrl+left_alt"] {
            assert!(input.parse::<PracticeShortcut>().is_err(), "{input}");
        }
        let configured: super::super::ShortcutConfig =
            toml::from_str("voice_practice_shortcut = 'right_ctrl+right_alt'").unwrap();
        let roundtrip: super::super::ShortcutConfig =
            toml::from_str(&toml::to_string(&configured).unwrap()).unwrap();
        assert_eq!(configured, roundtrip);
        assert_eq!(
            roundtrip.voice_practice_shortcut,
            PracticeShortcut::RIGHT_CTRL_ALT
        );
    }

    #[test]
    fn serde_round_trip_keeps_legacy_default() {
        let legacy: super::super::ShortcutConfig = toml::from_str("").unwrap();
        assert_eq!(legacy.voice_practice_shortcut, PracticeShortcut::DEFAULT);
        let configured: super::super::ShortcutConfig =
            toml::from_str("voice_practice_shortcut = 'ctrl+alt+e'").unwrap();
        let roundtrip: super::super::ShortcutConfig =
            toml::from_str(&toml::to_string(&configured).unwrap()).unwrap();
        assert_eq!(configured, roundtrip);
    }

    #[test]
    fn right_command_option_preserves_sides_and_normalizes_aliases() {
        for input in [
            "right_win+right_alt",
            "right_cmd+right_alt",
            " RIGHT_ALT + RIGHT_COMMAND ",
        ] {
            let shortcut: PracticeShortcut = input.parse().unwrap();
            assert_eq!(shortcut, PracticeShortcut::RIGHT_WIN_ALT);
            assert_eq!(shortcut.key_string(), "right_win+right_alt");
            assert!(shortcut.is_modifier_key(0x5c));
            assert!(shortcut.is_modifier_key(0xa5));
            assert!(!shortcut.is_modifier_key(0x5b));
            assert!(!shortcut.is_modifier_key(0xa4));
        }
        for input in ["cmd+right_alt", "right_cmd+left_alt", "cmd+alt"] {
            assert!(input.parse::<PracticeShortcut>().is_err(), "{input}");
        }
        let config: super::super::ShortcutConfig =
            toml::from_str("voice_practice_shortcut = 'right_cmd+right_alt'").unwrap();
        let roundtrip: super::super::ShortcutConfig =
            toml::from_str(&toml::to_string(&config).unwrap()).unwrap();
        assert_eq!(
            roundtrip.voice_practice_shortcut,
            PracticeShortcut::RIGHT_WIN_ALT
        );
    }
}
