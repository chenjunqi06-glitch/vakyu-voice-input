//! 离线候选窗测速：只计 `Renderer::render`，不包含字体初始化、候选查询或平台贴图。

mod args;
mod runner;
mod scenes;

use clap::Parser;

use args::Args;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "qingjian_render=warn".into()),
        )
        .init();
    let args = Args::parse();
    let report = runner::run(&args)?;
    print!("{report}");
    if let Some(path) = &args.report {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, report)?;
    }
    Ok(())
}
