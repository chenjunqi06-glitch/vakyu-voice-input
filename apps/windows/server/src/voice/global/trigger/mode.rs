//! 按下时确定的语音用途；跟读总开关不能改变本轮普通口述。

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VoiceMode {
    Dictation,

    Practice,
}
