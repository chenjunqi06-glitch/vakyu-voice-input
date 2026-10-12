//! 一套配色。缺省两套取自 macOS 系统语义色在 sRGB 下的实测值（label 0.847、secondaryLabel 0.498…）。

use crate::color::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    /// 候选词。
    pub text: Color,

    /// 译文。
    pub gloss: Color,

    /// 词性，比译文更浅。
    pub pos: Color,

    /// 生词译文：比普通译文醒目，看熟了就回到译文色。
    pub fresh: Color,

    /// 序号。
    pub index: Color,

    /// 云联想的云朵与文字：比译文醒目一点，但不抢候选词。
    pub cloud: Color,

    /// 窗口背景。
    pub background: Color,

    /// 可选的右下角渐变色；没有时用纯色背景。
    pub background_end: Option<Color>,

    /// 当前候选的高亮底色。
    pub highlight: Color,
}

impl Palette {
    pub const fn light() -> Self {
        Self {
            text: Color::gray(0, 216),
            gloss: Color::gray(0, 127),
            pos: Color::gray(0, 66),
            fresh: Color::rgb(255, 141, 40),
            index: Color::gray(0, 66),
            cloud: Color::rgb(0, 195, 208),
            background: Color::rgb(255, 255, 255),
            background_end: None,
            highlight: Color::rgba(176, 206, 125, 127),
        }
    }

    pub const fn dark() -> Self {
        Self {
            text: Color::gray(255, 216),
            gloss: Color::gray(255, 140),
            pos: Color::gray(255, 63),
            fresh: Color::rgb(255, 146, 48),
            index: Color::gray(255, 63),
            cloud: Color::rgb(0, 210, 224),
            background: Color::rgb(30, 30, 30),
            background_end: None,
            highlight: Color::rgba(36, 76, 36, 255),
        }
    }

    /// Windows 品牌主题：紫色渐变、白色正文，辅助文字保持可读的明度。
    pub const fn purple() -> Self {
        Self {
            text: Color::rgb(255, 255, 255),
            gloss: Color::rgb(229, 216, 248),
            pos: Color::rgb(220, 203, 243),
            fresh: Color::rgb(255, 228, 178),
            index: Color::rgb(220, 203, 243),
            cloud: Color::rgb(247, 238, 255),
            background: Color::rgb(109, 53, 181),
            background_end: Some(Color::rgb(53, 32, 99)),
            highlight: Color::rgba(255, 255, 255, 35),
        }
    }
}
