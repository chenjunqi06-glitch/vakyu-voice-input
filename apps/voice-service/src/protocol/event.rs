//! 发给原生界面的事件；不包含密钥或配置原文。

use qingjian_practice::PracticeView;
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    Ready,
    Config {
        voice_input: bool,
        practice: bool,
        learn_english: bool,
        send_translation: bool,
        demo: bool,
        shortcut: String,
        dictation_key: String,
        hold_ms: u32,
        similarity: u8,
        name_similarity: u8,
    },
    Capture {
        active: bool,
        generation: u64,
        language: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        practice_text: Option<String>,
    },
    Dictation {
        text: String,
        recording: bool,
        level: f32,
    },
    Practice {
        view: Box<PracticeView>,
    },
    Speak {
        id: u64,
        text: String,
        language: String,
        rate: i32,
    },
    CancelSpeech,
    Commit {
        id: u64,
        text: String,
    },
    Hide,
    Error {
        message: String,
    },
    /// 划词翻译结果：`ok` 时 `text` 是译文，否则是可读的失败提示；`target` 为 en / zh。
    Translation {
        id: u64,
        ok: bool,
        text: String,
        target: String,
    },
    SettingResult {
        key: String,
        ok: bool,
        message: String,
    },
}
