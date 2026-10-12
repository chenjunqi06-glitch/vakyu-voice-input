//! 逐场景计时与统计；结果消费和输出在计时区间之外，不将 PNG 编码混入渲染成本。

use std::error::Error;
use std::hint::black_box;
use std::io;
use std::time::{Duration, Instant};

use qingjian_render::{FontLibrary, Frame, Layout, RenderError, Renderer, Shadow, Theme};

use super::args::Args;
use super::scenes;

type Sample = (Duration, u32, u32);

pub(super) fn run(args: &Args) -> Result<String, Box<dyn Error>> {
    if !args.scale.is_finite() || args.scale <= 0.0 || args.scale > 4.0 {
        return Err(io::Error::other("scale 必须在 0 到 4 之间且不含 0").into());
    }
    let shadow = (!args.no_shadow).then_some(Shadow::mac_panel());
    let theme = Theme::purple();
    let fixed = scenes::fixed();
    let mut cold = Vec::with_capacity(args.cold_samples as usize);
    for _ in 0..args.cold_samples {
        let locale = args.locale.clone();
        let scale = args.scale;
        // 新线程让阴影 TLS 缓存为空，新 Renderer 让字形缓存为空；不清理操作系统文件缓存。
        let sample = std::thread::spawn(move || -> Result<Sample, String> {
            let library = FontLibrary::system(&locale).map_err(|error| error.to_string())?;
            let mut renderer = Renderer::new(library);
            timed(
                &mut renderer,
                &scenes::fixed(),
                &Theme::purple(),
                scale,
                shadow.as_ref(),
            )
            .map_err(|error| error.to_string())
        })
        .join()
        .map_err(|_| io::Error::other("冷帧线程异常退出"))?
        .map_err(io::Error::other)?;
        cold.push(sample);
    }

    let library = FontLibrary::system(&args.locale)?;
    let family = library.ui_family().to_owned();
    let mut renderer = Renderer::new(library);
    let mut same_frame = fixed.clone();
    for index in 0..20 {
        same_frame.highlighted = Some(index % same_frame.rows.len());
        timed(
            &mut renderer,
            &same_frame,
            &theme,
            args.scale,
            shadow.as_ref(),
        )?;
    }
    let mut same = Vec::with_capacity(args.samples as usize);
    for index in 0..args.samples {
        same_frame.highlighted = Some(index as usize % same_frame.rows.len());
        same.push(timed(
            &mut renderer,
            &same_frame,
            &theme,
            args.scale,
            shadow.as_ref(),
        )?);
    }

    let mut report = format!(
        "# Miracle 渲染测速\n\n环境：{} / {}；构建：{}；缩放：{}；界面字体：{}；主题：紫色渐变；阴影：{}。\n\n只测 `Renderer::render`，不含字体库初始化、线程创建、候选查询、网络请求、PNG 编码、RGBA→DIB 或 `UpdateLayeredWindow`。冷帧每次使用新线程和新 Renderer，操作系统字体文件缓存不清空。连续帧预热 20 帧并轮换高亮。逐键场景预先生成 1 到指定长度的 ASCII 拼音帧，九条固定中英候选，重复 {} 轮；长度每轮归零，不预热该场景。p95 使用最近秩（ceil(0.95 × n)）。\n\n| 场景 | 帧数 | 平均 ms | p95 ms | 最大 ms | 位图宽度范围 px | 位图高度范围 px |\n|---|---:|---:|---:|---:|---:|---:|\n",
        std::env::consts::OS,
        std::env::consts::ARCH,
        if cfg!(debug_assertions) {
            "Debug"
        } else {
            "Release"
        },
        args.scale,
        family,
        if args.no_shadow { "关闭" } else { "开启" },
        args.passes,
    );
    report.push_str(&row("冷帧", &cold));
    report.push_str(&row("同尺寸高亮轮换", &same));
    for keys in [51, 160] {
        let frames = scenes::growing(keys);
        let mut samples = Vec::with_capacity(frames.len() * args.passes as usize);
        for _ in 0..args.passes {
            for frame in &frames {
                samples.push(timed(
                    &mut renderer,
                    frame,
                    &theme,
                    args.scale,
                    shadow.as_ref(),
                )?);
            }
        }
        report.push_str(&row(&format!("逐键增长至 {keys} 键"), &samples));
    }
    report.push_str(&format!(
        "\n复现：`cargo run --release -p qingjian-render --example benchmark -- --scale {} --locale {} --cold-samples {} --samples {} --passes {}{}`。\n\n这些数值来自当前构建；未测旧版本，不据此声明相对旧版本的加速比例。系统负载和字体页缓存会影响结果。\n",
        args.scale, args.locale, args.cold_samples, args.samples, args.passes,
        if args.no_shadow { " --no-shadow" } else { "" },
    ));
    Ok(report)
}

fn timed(
    renderer: &mut Renderer,
    frame: &Frame,
    theme: &Theme,
    scale: f32,
    shadow: Option<&Shadow>,
) -> Result<Sample, RenderError> {
    let started = Instant::now();
    let rendered = renderer.render(black_box(frame), Layout::Vertical, theme, scale, shadow)?;
    let elapsed = started.elapsed();
    let size = (rendered.pixmap.width(), rendered.pixmap.height());
    black_box(&rendered);
    Ok((elapsed, size.0, size.1))
}

fn row(name: &str, samples: &[Sample]) -> String {
    let mut times: Vec<_> = samples
        .iter()
        .map(|sample| sample.0.as_secs_f64() * 1000.0)
        .collect();
    times.sort_by(f64::total_cmp);
    let average = times.iter().sum::<f64>() / times.len() as f64;
    let p95 = times[(times.len() * 95).div_ceil(100) - 1];
    let min_width = samples.iter().map(|sample| sample.1).min().unwrap();
    let max_width = samples.iter().map(|sample| sample.1).max().unwrap();
    let min_height = samples.iter().map(|sample| sample.2).min().unwrap();
    let max_height = samples.iter().map(|sample| sample.2).max().unwrap();
    format!(
        "| {name} | {} | {average:.3} | {p95:.3} | {:.3} | {min_width}–{max_width} | {min_height}–{max_height} |\n",
        times.len(),
        times.last().unwrap()
    )
}
