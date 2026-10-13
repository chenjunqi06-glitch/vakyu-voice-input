//! 状态条的一格。

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatusCell {
    /// 一段文字；`emphasized` 用品牌色（当前模式、生效中的全角标点），否则用译文的灰。
    Text { text: String, emphasized: bool },

    /// 功能开关；开启后显示高对比底色与粗体。
    Toggle { text: String, active: bool },

    /// 打开设置的齿轮。
    Gear,
}

impl StatusCell {
    pub fn toggle(text: impl Into<String>, active: bool) -> Self {
        Self::Toggle {
            text: text.into(),
            active,
        }
    }

    pub fn text(text: impl Into<String>, emphasized: bool) -> Self {
        Self::Text {
            text: text.into(),
            emphasized,
        }
    }
}
