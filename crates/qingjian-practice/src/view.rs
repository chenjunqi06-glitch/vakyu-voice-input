//! 双向跟读窗口的可序列化快照。
use crate::PracticeWordCard;
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PracticeView {
    /// 开启学英语，关闭学中文。
    pub learn_english: bool,

    /// 中文显示单元对应拼音；英文方向为空，在会话建立时计算一次。
    pub pronunciations: Vec<String>,

    pub original: String,

    pub words: Vec<String>,

    pub matched_words: usize,

    /// 本机当前用户的级别称号，例如「新手 Lv.1」。
    pub level_label: String,

    /// 当前等级的真实经验进度；由练习档案生成，界面不推算经验。
    pub level_progress: String,

    /// 本轮英语识别的临时文字，只在浮窗中预览，不记录或落盘。
    pub recognized: String,

    /// 本轮识别未通过时给出的简短反馈。
    pub feedback: String,

    /// 可操作时显示重试、跳过和手动放行按钮。
    pub actions_enabled: bool,

    /// 跟读麦克风已进入识别阶段；空识别文字时才提示继续朗读。
    pub recording: bool,

    /// 当前点击单词的本地词典结果；查询只更新这个浮层。
    pub word_card: Option<PracticeWordCard>,

    pub status: String,
}
