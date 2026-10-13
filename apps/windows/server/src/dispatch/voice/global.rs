//! 把语音所有权交给独立后台，并把配置热加载与安装前停止转发给它。

use super::NoopVoiceSink;
use super::shared_sink::SharedVoiceSink;
use crate::dispatch::Router;
use crate::voice::global::{GlobalVoice, TriggerConfig, VoiceConfig};

impl Router {
    pub(in crate::dispatch) fn save_practice_shortcut(
        &mut self,
        shortcut: &str,
    ) -> Result<(), String> {
        let shortcut = shortcut.parse::<qingjian_platform::PracticeShortcut>()?;
        if self.config.voice_key.conflicts_with_practice(shortcut) {
            return Err("该快捷键与普通语音转文字按键冲突，请更换；原快捷键保持不变".to_owned());
        }
        let path = self
            .config_path()
            .ok_or_else(|| "配置文件路径不可用，原快捷键保持不变".to_owned())?
            .to_owned();
        let global = self
            .global_voice
            .as_ref()
            .ok_or_else(|| "语音后台尚未就绪，原快捷键保持不变；请重启 Vakyu 后重试".to_owned())?;
        let started = std::time::Instant::now();
        let result = global.save_shortcut(shortcut, path);
        match &result {
            Ok(()) => tracing::info!(
                shortcut = %shortcut.label(),
                elapsed_ms = started.elapsed().as_millis(),
                "跟读快捷键已保存"
            ),
            Err(message) => tracing::warn!(
                shortcut = %shortcut.label(),
                elapsed_ms = started.elapsed().as_millis(),
                %message,
                "跟读快捷键保存失败"
            ),
        }
        result?;
        self.config.voice_practice_shortcut = shortcut;
        Ok(())
    }

    /// 取得进程单实例锁后调用；初始化失败时保留原有 TSF 语音入口。
    pub(crate) fn start_global_voice(&mut self) {
        if self.external_voice {
            return;
        }
        self.cancel_voice();
        let sink = SharedVoiceSink::new(std::mem::replace(
            &mut self.voice_sink,
            Box::new(NoopVoiceSink),
        ));
        match GlobalVoice::spawn(
            Box::new(sink.clone()),
            self.global_voice_config(),
            self.voice_delivery.clone(),
        ) {
            Ok(global) => {
                self.global_voice = Some(global);
                self.external_voice = true;
                tracing::info!("全局语音后台已接管按键，应用无需加载新版输入法组件");
            }
            Err(message) => {
                self.voice_sink = Box::new(sink);
                tracing::error!(%message, "全局语音启动失败，保留原输入法语音入口");
                self.prewarm_voice();
            }
        }
    }

    pub(in crate::dispatch) fn configure_global_voice(&self) {
        if let Some(global) = &self.global_voice {
            global.configure(self.global_voice_config());
        }
    }

    fn global_voice_config(&self) -> VoiceConfig {
        VoiceConfig {
            trigger: TriggerConfig {
                enabled: self.config.voice_input,
                key: self.config.voice_key,
                hold_ms: self.config.voice_hold_ms.clamp(100, 1000),
                practice_enabled: self.config.voice_english_practice,
                practice_shortcut: self.config.voice_practice_shortcut,
            },
            english_practice: self.config.voice_english_practice,
            learn_english: self.config.voice_practice_learn_english,
            send_translation: self.config.voice_practice_send_translation,
            practice_demo: self.config.voice_practice_demo,
            practice_match: qingjian_learning::PracticeMatchConfig {
                word_similarity: self.config.voice_practice_similarity,
                name_similarity: self.config.voice_practice_name_similarity,
            },
            translation: self.voice_translation.clone(),
        }
    }
}
