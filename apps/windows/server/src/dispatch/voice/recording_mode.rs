//! 录原话阶段的展示用途；与转写帧一起传递，音量重绘也保留同一入口标识。

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordingMode {
    Dictation,

    EnglishPractice,

    ChinesePractice,
}

impl RecordingMode {
    pub fn title(self) -> &'static str {
        match self {
            Self::Dictation => "普通语音",
            Self::EnglishPractice => "英语跟读",
            Self::ChinesePractice => "中文跟读",
        }
    }

    pub fn release_hint(self) -> &'static str {
        match self {
            Self::Dictation => "松开直接填入 · 不翻译",
            Self::EnglishPractice => "松开后翻译，再跟读英文",
            Self::ChinesePractice => "松开后翻译，再跟读中文",
        }
    }

    pub fn placeholder(self) -> &'static str {
        match self {
            Self::Dictation => "请说话，文字会实时显示在这里…",
            Self::EnglishPractice => "先说中文，再练习对应的英文句子…",
            Self::ChinesePractice => "Speak English, then practise Chinese…",
        }
    }
}
