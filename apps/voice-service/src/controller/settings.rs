//! 配置白名单、范围校验和热加载；错误消息不输出 TOML 或 API 凭据。

use super::Controller;
use crate::{game::PracticeGame, protocol::Event};
use qingjian_platform::{Config, PracticeShortcut};
use serde_json::Value;

pub fn validated_value(key: &str, value: &Value) -> Result<toml_edit::Value, &'static str> {
    match key {
        "voice_input"
        | "voice_english_practice"
        | "voice_practice_learn_english"
        | "voice_practice_demo" => value
            .as_bool()
            .map(Into::into)
            .ok_or("Expected a boolean setting"),
        "voice_hold_ms" => value
            .as_i64()
            .filter(|n| (100..=1000).contains(n))
            .map(Into::into)
            .ok_or("Hold delay must be 100–1000 ms"),
        "voice_practice_similarity" | "voice_practice_name_similarity" => value
            .as_i64()
            .filter(|n| (0..=100).contains(n))
            .map(Into::into)
            .ok_or("Similarity must be 0–100"),
        "voice_practice_shortcut" => value
            .as_str()
            .and_then(|s| s.parse::<PracticeShortcut>().ok())
            .map(|s| s.key_string().into())
            .ok_or("Unsupported shortcut"),
        _ => Err("This setting is not editable through the voice service"),
    }
}

impl Controller {
    pub(super) fn emit_config(&mut self) {
        let s = &self.config.shortcut;
        self.emit(Event::Config {
            voice_input: s.voice_input,
            practice: s.voice_english_practice,
            learn_english: s.voice_practice_learn_english,
            demo: s.voice_practice_demo,
            shortcut: s.voice_practice_shortcut.key_string(),
            dictation_key: match s.voice_key {
                qingjian_platform::VoiceKey::Alt => "alt",
                qingjian_platform::VoiceKey::LeftAlt => "left_alt",
                qingjian_platform::VoiceKey::RightAlt => "right_alt",
            }
            .to_owned(),
            hold_ms: s.voice_hold_ms(),
            similarity: s.voice_practice_similarity,
            name_similarity: s.voice_practice_name_similarity,
        });
    }

    pub fn reload(&mut self) {
        match crate::configuration::load(&self.config_path) {
            Ok(config) => self.apply_config(config),
            Err(_) => self.emit(Event::Error {
                message: self.message(
                    "配置读取失败，继续使用上次设置",
                    "Could not read configuration; keeping previous settings",
                ),
            }),
        }
    }

    pub(super) fn apply_config(&mut self, config: Config) {
        if config.shortcut == self.config.shortcut && config.predict == self.config.predict {
            return;
        }
        let direction_changed =
            config.shortcut.voice_practice_learn_english != self.learn_english();
        let mode_changed = config.shortcut.voice_input != self.config.shortcut.voice_input
            || (self.round_mode == crate::protocol::Mode::Practice
                && (direction_changed
                    || config.shortcut.voice_english_practice
                        != self.config.shortcut.voice_english_practice));
        let translation_changed = config.predict != self.config.predict;
        if mode_changed && (self.active || self.practice.is_some()) {
            self.requires_new_round = true;
            self.game.cancel();
            self.fail(self.message("学习方向或语音开关已改变；原话保留在窗口，请按 Esc 后重新录音",
                "Learning direction or voice mode changed; your text is kept here. Press Esc to start again"));
        } else if translation_changed
            && self
                .practice
                .as_ref()
                .is_some_and(|session| session.phase == qingjian_practice::Phase::Translating)
        {
            self.fail(self.message(
                "翻译设置已更新，请点“继续补读”使用新设置；原话尚未填入",
                "Translation settings updated; choose Continue to use them. No text was inserted",
            ));
        }
        if translation_changed {
            if let Some(translator) = &mut self.translator {
                translator.cancel();
            }
            self.translator = None;
        }
        self.config = config;
        if direction_changed {
            self.game = PracticeGame::load(&self.data, self.learn_english());
        }
        self.emit_config();
    }

    pub(super) fn set(&mut self, key: &str, value: Value) {
        let result = validated_value(key, &value).and_then(|value| {
            Config::set_value(&self.config_path, "shortcut", key, value)
                .map_err(|_| "Could not save configuration")
        });
        match result {
            Ok(()) => {
                self.reload();
                self.emit(Event::SettingResult {
                    key: key.to_owned(),
                    ok: true,
                    message: self.message("已保存", "Saved"),
                });
            }
            Err(error) => self.emit(Event::SettingResult {
                key: key.to_owned(),
                ok: false,
                message: error.to_owned(),
            }),
        }
    }
}
