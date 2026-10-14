//! 快捷键与语音练习的持久化配置，旧文件沿用各字段的默认值。

use qingjian_core::ModeKeys;
use serde::{Deserialize, Serialize};

use super::key_combo::KeyCombo;
use super::modifiers::Modifiers;
use super::practice_shortcut::PracticeShortcut;
use super::switch_key::SwitchKeys;
use super::voice_key::VoiceKey;

/// 配置文件 `[shortcut]` 分节：前缀模式键（Core 的 [`ModeKeys`]）加壳层的修饰键组合。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShortcutConfig {
    /// 表达式 / 问字模式键，键名与以前一样直接在分节下（`expression` / `question`）。
    #[serde(flatten)]
    pub mode: ModeKeys,

    /// 中 / 英切换键（Windows 用），可多选：`["shift", "control", "ctrl+alt+space"]`。详见 [`SwitchKeys`]。
    pub switch_mode: SwitchKeys,

    /// Windows 按住 Alt 说话，松开上屏。
    pub voice_input: bool,

    /// 中文口述后先练习英文，读完整句才填入原中文；默认关闭。
    pub voice_english_practice: bool,

    /// 跟读方向：开启学英语（中→英），关闭学中文（英→中）；独立于练习总开关。
    pub voice_practice_learn_english: bool,

    /// 跟读读完后填入译文而不是原话（说中文、读英文、发英文）；只在跟读练习开启时生效，默认关闭。
    pub voice_practice_send_translation: bool,

    /// 英语跟读前先在本机慢速示范朗读；只在跟读练习开启时生效。
    pub voice_practice_demo: bool,

    /// Windows：英语示范用随包的自然人声（本机神经网络语音）；关闭或组件缺失时用系统声音。
    pub voice_practice_natural_voice: bool,

    /// 自然人声的音色编号（随包英语模型有 904 个说话人，0–903）；超出范围按默认处理。
    pub voice_practice_speaker: u16,

    /// 开启英语跟读时的按住录音键；独立于普通语音的 Alt 设置。
    pub voice_practice_shortcut: PracticeShortcut,

    /// 新手英语跟读普通词的最低相似度百分比；高等级会逐步收紧。
    pub voice_practice_similarity: u8,

    /// 中文原句确认的人名/专名最低相似度百分比，仅新手级使用。
    pub voice_practice_name_similarity: u8,

    /// 语音按住键：两侧 Alt、左 Alt 或右 Alt。
    pub voice_key: VoiceKey,

    /// 开始录音前的按住时长，读取时规范到 100–1000 毫秒。
    pub voice_hold_ms: u32,

    /// 数字键配这些修饰键：上屏候选的第一个译词。
    pub translation: Modifiers,

    /// 数字键配这些修饰键：上屏候选的第二个译词（候选右侧有两个译词时）。
    pub translation_second: Modifiers,

    /// 把应用里选中的文字译成学习语言（需要云服务开着）。
    pub translate_selection: KeyCombo,

    /// Windows：在任意软件里选中文字后，鼠标旁浮出「译」小图标，点它弹出译文；用本机离线模型翻译。
    pub translate_selection_icon: bool,

    /// 数字键配这些修饰键：删掉候选（用户词整个删掉，词库词清掉对它的学习）。
    pub delete_candidate: Modifiers,
}

/// 自然人声的默认音色：中等音高、吐字清楚的女声（编号 180，试听后选定）。
pub const DEFAULT_PRACTICE_SPEAKER: u16 = 180;

impl Default for ShortcutConfig {
    fn default() -> Self {
        // Windows 上 Alt+数字被系统当菜单快捷键截走（TSF 收不到），译词键缺省用 Ctrl；macOS 用 Option。
        #[cfg(windows)]
        let (translation, translation_second) = (Modifiers::CONTROL, Modifiers::SHIFT_CONTROL);
        #[cfg(not(windows))]
        let (translation, translation_second) = (Modifiers::OPTION, Modifiers::SHIFT_OPTION);
        Self {
            mode: ModeKeys::default(),
            switch_mode: SwitchKeys::default(),
            voice_input: true,
            voice_english_practice: false,
            voice_practice_learn_english: true,
            voice_practice_send_translation: false,
            voice_practice_demo: true,
            voice_practice_natural_voice: true,
            voice_practice_speaker: DEFAULT_PRACTICE_SPEAKER,
            voice_practice_shortcut: if cfg!(windows) {
                PracticeShortcut::RIGHT_CTRL_ALT
            } else {
                PracticeShortcut::default()
            },
            voice_practice_similarity: 78,
            voice_practice_name_similarity: 45,
            voice_key: if cfg!(windows) {
                VoiceKey::LeftAlt
            } else {
                VoiceKey::Alt
            },
            voice_hold_ms: 100,
            translation,
            translation_second,
            translate_selection: KeyCombo::TRANSLATE_DEFAULT,
            translate_selection_icon: true,
            delete_candidate: Modifiers::SHIFT,
        }
    }
}

impl ShortcutConfig {
    pub fn voice_hold_ms(&self) -> u32 {
        self.voice_hold_ms.clamp(100, 1000)
    }

    /// 删候选的修饰键；为空或与任一组译词键撞了就退回缺省。
    pub fn delete_keys(&self) -> Modifiers {
        let (first, second) = self.translation_keys();
        if self.delete_candidate.is_empty()
            || self.delete_candidate == first
            || self.delete_candidate == second
        {
            Self::default().delete_candidate
        } else {
            self.delete_candidate
        }
    }

    /// 两组译词修饰键；两组相同或有一组为空时整个退回缺省，不做一半。
    pub fn translation_keys(&self) -> (Modifiers, Modifiers) {
        if self.translation == self.translation_second
            || self.translation.is_empty()
            || self.translation_second.is_empty()
        {
            let default = Self::default();
            (default.translation, default.translation_second)
        } else {
            (self.translation, self.translation_second)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voice_keys_use_platform_defaults_preserve_explicit_values_and_clamp_bounds() {
        let default: ShortcutConfig = toml::from_str("").unwrap();
        assert!(!default.voice_english_practice);
        assert!(default.voice_practice_demo);
        assert_eq!(
            default.voice_key,
            if cfg!(windows) {
                VoiceKey::LeftAlt
            } else {
                VoiceKey::Alt
            }
        );
        assert_eq!(
            default.voice_practice_shortcut,
            if cfg!(windows) {
                PracticeShortcut::RIGHT_CTRL_ALT
            } else {
                PracticeShortcut::default()
            }
        );
        let explicit: ShortcutConfig =
            toml::from_str("voice_key = \"alt\"\nvoice_practice_shortcut = \"f8\"\n").unwrap();
        assert_eq!(explicit.voice_key, VoiceKey::Alt);
        assert_eq!(explicit.voice_practice_shortcut, "f8".parse().unwrap());
        assert_eq!(default.voice_hold_ms(), 100);
        for (name, key) in [
            ("alt", VoiceKey::Alt),
            ("left_alt", VoiceKey::LeftAlt),
            ("right_alt", VoiceKey::RightAlt),
        ] {
            let parsed: ShortcutConfig =
                toml::from_str(&format!("voice_key = \"{name}\"\nvoice_hold_ms = 0\n")).unwrap();
            assert_eq!(parsed.voice_key, key);
            assert_eq!(parsed.voice_hold_ms(), 100);
        }
        let long: ShortcutConfig = toml::from_str("voice_hold_ms = 99999").unwrap();
        assert_eq!(long.voice_hold_ms(), 1000);
    }

    #[test]
    fn english_practice_requires_explicit_opt_in_and_survives_round_trip() {
        let legacy: ShortcutConfig =
            toml::from_str("voice_input = true\nvoice_key = \"right_alt\"\n").unwrap();
        assert!(!legacy.voice_english_practice);
        assert!(legacy.voice_practice_demo);

        let enabled: ShortcutConfig = toml::from_str("voice_english_practice = true\n").unwrap();
        assert!(enabled.voice_english_practice);
        let restored: ShortcutConfig = toml::from_str(&toml::to_string(&enabled).unwrap()).unwrap();
        assert_eq!(restored, enabled);

        let disabled: ShortcutConfig = toml::from_str("voice_english_practice = false\n").unwrap();
        assert!(!disabled.voice_english_practice);
        assert!(disabled.voice_input);
    }

    #[test]
    fn practice_direction_defaults_to_english_and_does_not_enable_practice() {
        let legacy: ShortcutConfig = toml::from_str("").unwrap();
        assert!(legacy.voice_practice_learn_english);
        let chinese: ShortcutConfig =
            toml::from_str("voice_practice_learn_english = false").unwrap();
        assert!(!chinese.voice_english_practice);
        assert!(!chinese.voice_practice_learn_english);
        let restored: ShortcutConfig = toml::from_str(&toml::to_string(&chinese).unwrap()).unwrap();
        assert_eq!(chinese, restored);
    }

    #[test]
    fn send_translation_defaults_off_and_round_trips() {
        let legacy: ShortcutConfig = toml::from_str("voice_english_practice = true\n").unwrap();
        assert!(!legacy.voice_practice_send_translation);
        assert!(legacy.translate_selection_icon);
        let enabled: ShortcutConfig = toml::from_str(
            "voice_practice_send_translation = true\ntranslate_selection_icon = false\n",
        )
        .unwrap();
        assert!(enabled.voice_practice_send_translation);
        assert!(!enabled.voice_english_practice);
        assert!(!enabled.translate_selection_icon);
        let restored: ShortcutConfig = toml::from_str(&toml::to_string(&enabled).unwrap()).unwrap();
        assert_eq!(restored, enabled);
    }

    #[test]
    fn natural_voice_defaults_on_and_round_trips() {
        let legacy: ShortcutConfig = toml::from_str("voice_practice_demo = true\n").unwrap();
        assert!(legacy.voice_practice_natural_voice);
        assert_eq!(
            legacy.voice_practice_speaker,
            super::DEFAULT_PRACTICE_SPEAKER
        );
        let system: ShortcutConfig =
            toml::from_str("voice_practice_natural_voice = false\nvoice_practice_speaker = 700\n")
                .unwrap();
        assert!(!system.voice_practice_natural_voice);
        assert_eq!(system.voice_practice_speaker, 700);
        let restored: ShortcutConfig = toml::from_str(&toml::to_string(&system).unwrap()).unwrap();
        assert_eq!(restored, system);
    }

    #[test]
    fn practice_demo_defaults_on_without_enabling_practice_and_can_be_disabled() {
        let legacy: ShortcutConfig = toml::from_str("voice_english_practice = false\n").unwrap();
        assert!(legacy.voice_practice_demo);
        assert!(!legacy.voice_english_practice);
        let disabled: ShortcutConfig = toml::from_str("voice_practice_demo = false\n").unwrap();
        let restored: ShortcutConfig =
            toml::from_str(&toml::to_string(&disabled).unwrap()).unwrap();
        assert_eq!(restored, disabled);
        assert!(!restored.voice_practice_demo);
        assert!(!restored.voice_english_practice);
    }

    #[test]
    fn practice_match_thresholds_have_safe_defaults_and_round_trip() {
        let legacy: ShortcutConfig = toml::from_str("voice_english_practice = true\n").unwrap();
        assert_eq!(legacy.voice_practice_similarity, 78);
        assert_eq!(legacy.voice_practice_name_similarity, 45);

        let configured: ShortcutConfig =
            toml::from_str("voice_practice_similarity = 82\nvoice_practice_name_similarity = 42\n")
                .unwrap();
        let restored: ShortcutConfig =
            toml::from_str(&toml::to_string(&configured).unwrap()).unwrap();
        assert_eq!(restored.voice_practice_similarity, 82);
        assert_eq!(restored.voice_practice_name_similarity, 42);
    }

    #[test]
    fn old_files_without_modifier_keys_still_parse_and_get_defaults() {
        // 缺省值分平台（Windows 用 Ctrl 系、其余用 Option 系），断言跟着平台的 Default 走
        let default = ShortcutConfig::default();
        let parsed: ShortcutConfig = toml::from_str("expression = \"i\"\n").unwrap();
        assert_eq!(parsed.mode.expression, 'i');
        assert_eq!(parsed.switch_mode, SwitchKeys::default());
        assert_eq!(
            parsed.translation_keys(),
            (default.translation, default.translation_second)
        );
        let same: ShortcutConfig =
            toml::from_str("translation = \"option\"\ntranslation_second = \"option\"\n").unwrap();
        assert_eq!(
            same.translation_keys(),
            (default.translation, default.translation_second)
        );
        let swapped: ShortcutConfig =
            toml::from_str("translation = \"control+option\"\ntranslation_second = \"option\"\n")
                .unwrap();
        assert_eq!(swapped.translation_keys().1, Modifiers::OPTION);
    }

    #[test]
    fn delete_keys_fall_back_when_clashing_with_translation_keys() {
        let default = ShortcutConfig::default();
        let parsed: ShortcutConfig = toml::from_str("").unwrap();
        assert_eq!(parsed.delete_keys(), default.delete_candidate);
        // 与平台缺省的译词键撞上才算「冲突」，两边平台都成立
        let clash: ShortcutConfig = toml::from_str(&format!(
            "delete_candidate = \"{}\"\n",
            default.translation.key()
        ))
        .unwrap();
        assert_eq!(clash.delete_keys(), default.delete_candidate);
        // 不与任何一组译词键冲突的修饰键：平台上取一个，断言它原样生效
        let free = [Modifiers::OPTION, Modifiers::CONTROL, Modifiers::SHIFT]
            .into_iter()
            .find(|m| *m != default.translation && *m != default.translation_second)
            .unwrap();
        let custom: ShortcutConfig =
            toml::from_str(&format!("delete_candidate = \"{}\"\n", free.key())).unwrap();
        assert_eq!(custom.delete_keys(), free);
    }

    #[test]
    fn switch_mode_parses_and_defaults_to_shift() {
        let parsed: ShortcutConfig = toml::from_str("switch_mode = [\"ctrl\"]\n").unwrap();
        assert!(parsed.switch_mode.control && !parsed.switch_mode.shift);
        let off: ShortcutConfig = toml::from_str("switch_mode = \"none\"\n").unwrap();
        assert_eq!(off.switch_mode, crate::SwitchKeys::NONE);
        let missing: ShortcutConfig = toml::from_str("").unwrap();
        assert_eq!(missing.switch_mode, SwitchKeys::default());
    }
}
