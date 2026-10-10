//! 原生宿主提供明确的配置、数据、Python 和模型目录。

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
pub struct Arguments {
    #[arg(long)]
    pub config: PathBuf,

    #[arg(long)]
    pub data: PathBuf,

    #[arg(long)]
    pub python: PathBuf,

    #[arg(long)]
    pub runtime: PathBuf,

    #[arg(long)]
    pub models: PathBuf,
}
