//! 托盘开关与配置热加载共用同一条应用路径；写入失败时保留当前状态。

mod action;
mod state;

#[cfg(test)]
mod tests;

pub use action::TrayAction;
pub use state::TrayState;

use qingjian_platform::Config;
use qingjian_platform::protocol::IndicatorCommand;

use crate::dispatch::Router;

impl Router {
    pub(in crate::dispatch) fn tray_state(&self) -> TrayState {
        TrayState {
            voice_input: self.config.voice_input,
            english_practice: self.config.voice_english_practice,
            learn_english: self.config.voice_practice_learn_english,
            practice_demo: self.config.voice_practice_demo,
            cloud: self.config.cloud_enabled,
            status_bar: self.config.status_enabled,
            full_width: self.full_width_for(self.english),
        }
    }

    pub(in crate::dispatch) fn handle_tray(&mut self, action: TrayAction) {
        let setting = match action {
            TrayAction::ToggleVoiceInput => ("shortcut", "voice_input", !self.config.voice_input),
            TrayAction::ToggleEnglishPractice => (
                "shortcut",
                "voice_english_practice",
                !self.config.voice_english_practice,
            ),
            TrayAction::TogglePracticeDemo => (
                "shortcut",
                "voice_practice_demo",
                !self.config.voice_practice_demo,
            ),
            TrayAction::ToggleLearningDirection => (
                "shortcut",
                "voice_practice_learn_english",
                !self.config.voice_practice_learn_english,
            ),
            TrayAction::ToggleCloud => ("predict", "enabled", !self.config.cloud_enabled),
            TrayAction::ToggleStatusBar => ("status_bar", "enabled", !self.config.status_enabled),
            TrayAction::TogglePunctuation => {
                self.handle_indicator(IndicatorCommand::TogglePunctuation);
                return;
            }
            TrayAction::OpenSettings => {
                self.status.open_settings();
                return;
            }
        };
        // 菜单被禁用的项在这里也拦住，避免旧菜单状态下启用无效组合。
        if matches!(action, TrayAction::ToggleEnglishPractice) && !self.config.voice_input
            || matches!(action, TrayAction::TogglePracticeDemo)
                && (!self.config.voice_input || !self.config.voice_english_practice)
        {
            return;
        }
        if let Some(path) = self.config_path().map(ToOwned::to_owned) {
            // 先读取完整配置再写单键：保留用户正在设置页改的其他项，并复用热加载的取消录音逻辑。
            let loaded = Config::load(&path).and_then(|_| {
                Config::set_value(&path, setting.0, setting.1, setting.2)?;
                Config::load(&path)
            });
            match loaded {
                Ok(config) => self.apply_config(&config),
                Err(error) => tracing::warn!(%error, "托盘开关保存失败，保持当前配置"),
            }
            return;
        }
        // 无配置文件的嵌入宿主 / 测试只变更内存，不创建另一个配置目录。
        match action {
            TrayAction::ToggleVoiceInput => self.config.voice_input = setting.2,
            TrayAction::ToggleEnglishPractice => self.config.voice_english_practice = setting.2,
            TrayAction::ToggleLearningDirection => {
                self.config.voice_practice_learn_english = setting.2
            }
            TrayAction::TogglePracticeDemo => self.config.voice_practice_demo = setting.2,
            TrayAction::ToggleCloud => self.config.cloud_enabled = setting.2,
            TrayAction::ToggleStatusBar => self.config.status_enabled = setting.2,
            _ => unreachable!(),
        }
        self.expire_voice();
        #[cfg(windows)]
        self.configure_global_voice();
        self.reconcile_status();
    }
}
