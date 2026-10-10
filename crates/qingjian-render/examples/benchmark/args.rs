//! 候选窗测速参数，所有场景在给定缩放下使用相同紫色主题和候选内容。

use std::path::PathBuf;

use clap::Parser;

#[derive(Parser)]
pub(super) struct Args {
    /// 点 → 像素倍数。
    #[arg(long, default_value_t = 1.25)]
    pub scale: f32,

    /// 字体回退 locale。
    #[arg(long, default_value = "zh-CN")]
    pub locale: String,

    /// 独立线程和新渲染器的冷帧次数。
    #[arg(long, default_value_t = 20, value_parser = clap::value_parser!(u32).range(1..=1000))]
    pub cold_samples: u32,

    /// 预热后同尺寸连续帧次数。
    #[arg(long, default_value_t = 500, value_parser = clap::value_parser!(u32).range(1..=100_000))]
    pub samples: u32,

    /// 逐键增长场景的重复轮数。
    #[arg(long, default_value_t = 5, value_parser = clap::value_parser!(u32).range(1..=1000))]
    pub passes: u32,

    /// 不绘制阴影，便于单独核对同构建的阴影成本。
    #[arg(long)]
    pub no_shadow: bool,

    /// 保存 Markdown 结果；缺省仅写到标准输出。
    #[arg(long)]
    pub report: Option<PathBuf>,
}
