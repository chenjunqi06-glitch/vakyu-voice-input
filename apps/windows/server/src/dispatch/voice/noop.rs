//! 无界面环境的语音输出端。

use qingjian_platform::protocol::ScreenRect;

use super::sink::VoiceSink;

pub struct NoopVoiceSink;

impl VoiceSink for NoopVoiceSink {
    fn show_voice(&self, _text: String, _rect: ScreenRect) {}

    fn hide_voice(&self) {}
}
