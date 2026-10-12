//! 临时语音浮窗的输出端，不依赖 Windows 类型。

use qingjian_platform::protocol::ScreenRect;

use super::PracticeView;

pub trait VoiceSink: Send {
    /// 全局按键被暂扣时由后台续约显示许可；旧 TSF 入口不调用，仍按原来的 Alt 状态判断。
    fn set_voice_held(&self, _held: bool) {}

    /// 真实麦克风采样的音量包络（0–100）；不承载识别文字。
    fn set_voice_level(&self, _level: u8) {}

    fn prepare_voice(&self, rect: ScreenRect) {
        self.show_voice("正在准备麦克风…".to_owned(), rect);
    }

    fn show_voice(&self, text: String, rect: ScreenRect);

    fn show_practice(&self, view: PracticeView, rect: ScreenRect) {
        self.show_voice(format!("{}\n{}", view.original, view.words.join(" ")), rect);
    }

    fn voice_error(&self, message: String, rect: ScreenRect) {
        self.show_voice(message, rect);
    }

    fn hide_voice(&self);
}
