//! 紫色渐变设置外壳：原生控件保持键盘与无障碍行为，品牌图与背景内嵌随程序部署。

mod background;
mod navigation;

use std::{path::PathBuf, sync::OnceLock};
use windows_reactor::{
    Border, ChildrenControl, Color, ContentControl, Grid, GridChildExt, GridLength, LayoutControl,
    View, ViewContext, WindowConstraints, WindowTheme, WindowVisuals,
};

use super::Settings;

pub(super) const TEXT: Color = Color::rgb(255, 255, 255);
pub(super) const SECONDARY_TEXT: Color = Color::rgb(229, 216, 248);

pub(super) fn view(settings: &Settings, context: &mut ViewContext<Settings>) -> View {
    context.window_title("Vakyu 设置");
    let mut visuals = WindowVisuals::new()
        .theme(WindowTheme::Dark)
        .client_size(1060.0, 780.0)
        .constraints(WindowConstraints {
            min_width: Some(840.0),
            min_height: Some(620.0),
            ..WindowConstraints::default()
        });
    if let Some(icon) = icon_path() {
        visuals = visuals.icon(icon);
    }
    context.window_visuals(visuals);
    let content = Grid::new()
        .columns([GridLength::Pixel(218.0), GridLength::STAR])
        .children([
            navigation::view(settings, context),
            Border::new()
                .grid_column(1)
                .margin(16.0)
                .corner_radius(24.0)
                .background(Color::argb(42, 30, 12, 59))
                .border_brush(Color::argb(30, 255, 255, 255))
                .border_thickness(1.0)
                .content(settings.page_content(context)),
        ]);
    Grid::new().children([background::view(), content])
}

fn icon_path() -> Option<&'static str> {
    static ICON_PATH: OnceLock<Option<&'static str>> = OnceLock::new();
    *ICON_PATH.get_or_init(|| {
        let installed_icon = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|parent| parent.join("vakyu.ico")))
            .filter(|path| path.is_file());
        let source_icon = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../assets/icon/windows/vakyu.ico")
            .canonicalize()
            .ok();
        let path = installed_icon.or(source_icon.filter(|path| path.is_file()))?;
        Some(Box::leak(
            path.to_string_lossy().into_owned().into_boxed_str(),
        ))
    })
}
