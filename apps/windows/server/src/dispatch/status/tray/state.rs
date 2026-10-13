//! 托盘菜单所需的轻量状态，不携带服务地址或密钥。

/// 每次配置变化后由 Router 发布，菜单打开时直接读取。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TrayState {
    pub voice_input: bool,

    pub english_practice: bool,

    pub learn_english: bool,

    /// 跟读读完后填入译文（说中文、读英文、发英文）。
    pub send_translation: bool,

    pub practice_demo: bool,

    pub cloud: bool,

    pub status_bar: bool,

    pub full_width: bool,
}
