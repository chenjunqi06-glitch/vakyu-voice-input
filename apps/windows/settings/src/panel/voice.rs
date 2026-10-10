//! 普通语音按键独立保存；与跟读入口重叠时保留原配置并在对应控件旁提示。

use qingjian_platform::{Config, VoiceKey};

use super::Settings;

impl Settings {
    pub(super) fn save_voice_key(&mut self, key: VoiceKey, value: &str) {
        self.reload();
        if key == self.config.shortcut.voice_key {
            return;
        }
        let practice = self.config.shortcut.voice_practice_shortcut;
        if key.conflicts_with_practice(practice) {
            self.voice_key_revision = self.voice_key_revision.wrapping_add(1);
            self.voice_key_notice.fail(format!(
                "该按键与口语跟读的 {} 冲突，原设置保持不变。请选择另一侧 Alt，或先修改跟读快捷键。",
                practice.label()
            ));
            return;
        }
        if let Err(error) = Config::set_value(&self.path, "shortcut", "voice_key", value) {
            self.voice_key_revision = self.voice_key_revision.wrapping_add(1);
            self.voice_key_notice
                .fail(format!("普通语音按键保存失败，原设置保持不变：{error}"));
            return;
        }
        self.reload();
        self.voice_key_notice
            .succeed("普通语音按键已保存，后台自动生效；开启跟读也可以独立转文字。".to_owned());
    }
}
