//! 品牌侧栏与原生导航按钮；选中态采用浅紫玻璃底，正文始终白色。

use windows_reactor::{
    Border, Button, ChildrenControl, Color, ContentControl, EncodedImage, FontWeight, Grid,
    GridChildExt, GridLength, HorizontalAlignment, Image, LayoutControl, Orientation,
    ResourceOverrides, ScrollViewer, StackPanel, Stretch, Symbol, SymbolIcon, TextBlock, Thickness,
    View, ViewContext,
};

use super::{SECONDARY_TEXT, TEXT};
use crate::panel::{Message, Settings};

const LOGO: &[u8] = include_bytes!("../../../../../../assets/icon/windows/vakyu-logo.png");

pub(super) fn view(settings: &Settings, context: &mut ViewContext<Settings>) -> View {
    let items = [
        ("general", "通用", Symbol::Setting),
        ("candidates", "候选窗口", Symbol::View),
        ("shortcut", "语音与快捷键", Symbol::Microphone),
        ("cloud", "云服务", Symbol::World),
        ("fuzzy", "模糊音", Symbol::Audio),
        ("dictionaries", "词库", Symbol::Library),
        ("aux_code", "辅码", Symbol::Character),
        ("usage", "统计", Symbol::List),
        ("advanced", "高级", Symbol::Repair),
        ("about", "关于", Symbol::Help),
    ];
    let buttons = items.map(|(tag, label, icon)| {
        let selected = settings.page == tag;
        let resources = ResourceOverrides::new()
            .set(
                "ButtonBackground",
                Color::argb(if selected { 42 } else { 0 }, 255, 255, 255),
            )
            .set(
                "ButtonBackgroundPointerOver",
                Color::argb(52, 255, 255, 255),
            )
            .set("ButtonBackgroundPressed", Color::argb(68, 255, 255, 255))
            .set(
                "ButtonBorderBrush",
                Color::argb(if selected { 52 } else { 0 }, 255, 255, 255),
            )
            .set(
                "ButtonBorderBrushPointerOver",
                Color::argb(66, 255, 255, 255),
            )
            .set("ButtonForeground", TEXT)
            .set("ButtonForegroundPointerOver", TEXT)
            .set("ButtonForegroundPressed", TEXT)
            .set(
                "ControlCornerRadius",
                windows_reactor::CornerRadius::uniform(10.0),
            );
        Button::new()
            .height(42.0)
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .horizontal_content_alignment(HorizontalAlignment::Stretch)
            .resource_overrides(resources)
            .on_click(context.message(Message::Navigate(Some(tag.to_owned()))))
            .content(
                StackPanel::new()
                    .orientation(Orientation::Horizontal)
                    .spacing(14.0)
                    .children((
                        SymbolIcon::new().symbol(icon).width(20.0),
                        TextBlock::new()
                            .text(label)
                            .foreground(TEXT)
                            .font_weight(if selected {
                                FontWeight::SEMI_BOLD
                            } else {
                                FontWeight::NORMAL
                            }),
                    )),
            )
    });
    Border::new()
        .background(Color::argb(48, 27, 11, 54))
        .border_brush(Color::argb(22, 255, 255, 255))
        .border_thickness(Thickness::new(0.0, 0.0, 1.0, 0.0))
        .padding(Thickness::new(16.0, 24.0, 16.0, 18.0))
        .content(
            Grid::new()
                .rows([GridLength::Auto, GridLength::STAR, GridLength::Auto])
                .children([
                    brand(),
                    Border::new()
                        .grid_row(1)
                        .margin(Thickness::new(0.0, 24.0, 0.0, 16.0))
                        .content(
                            ScrollViewer::new()
                                .content(StackPanel::new().spacing(6.0).children(buttons)),
                        ),
                    TextBlock::new()
                        .grid_row(2)
                        .text("设置会自动保存")
                        .foreground(SECONDARY_TEXT)
                        .font_size(12.0)
                        .horizontal_alignment(HorizontalAlignment::Center)
                        .into(),
                ]),
        )
}

fn brand() -> View {
    StackPanel::new()
        .spacing(6.0)
        .horizontal_alignment(HorizontalAlignment::Center)
        .children((
            Image::new()
                .source_data(EncodedImage::from_static(LOGO))
                .stretch(Stretch::Uniform)
                .width(174.0)
                .height(64.0),
            TextBlock::new()
                .text("让表达更自由")
                .font_size(12.0)
                .foreground(SECONDARY_TEXT)
                .horizontal_alignment(HorizontalAlignment::Center),
        ))
}
