//! 融入渐变背景的窗口操作区；拖动区与原生按钮分开，保留键盘和无障碍操作。

use windows_reactor::{
    AutomationExt, Border, Button, ChildrenControl, Color, ContentControl, CornerRadius,
    EncodedImage, FontIcon, Grid, GridChildExt, GridLength, Image, LayoutControl, Orientation,
    ResourceOverrides, StackPanel, Stretch, TextBlock, Thickness, VerticalAlignment, View,
    ViewContext,
};

use super::{CAPTION_BUTTONS_WIDTH, HEADER_HEIGHT, TEXT};
use crate::panel::{Message, Settings};

const ICON: &[u8] = include_bytes!("../../../../../../assets/icon/windows/vakyu-icon.png");

pub(super) fn view(context: &ViewContext<Settings>) -> View {
    let drag = Border::new()
        .background(Color::transparent())
        .padding(Thickness::new(18.0, 0.0, 0.0, 0.0))
        .on_pointer_pressed(
            context.callback(|event: windows_reactor::PointerEventInfo| {
                Message::WindowDrag(event.is_left_button_pressed)
            }),
        )
        .content(
            StackPanel::new()
                .orientation(Orientation::Horizontal)
                .spacing(9.0)
                .vertical_alignment(VerticalAlignment::Center)
                .children((
                    Image::new()
                        .source_data(EncodedImage::from_static(ICON))
                        .stretch(Stretch::Uniform)
                        .width(24.0)
                        .height(24.0),
                    TextBlock::new()
                        .text("Vakyu 设置")
                        .foreground(TEXT)
                        .font_size(13.0)
                        .vertical_alignment(VerticalAlignment::Center),
                )),
        );
    let buttons = [
        ("最小化", "\u{e921}", Message::WindowMinimize, false),
        ("最大化或还原", "\u{e922}", Message::WindowMaximize, false),
        ("关闭设置", "\u{e8bb}", Message::WindowClose, true),
    ]
    .map(|(label, glyph, message, close)| {
        let resources = ResourceOverrides::new()
            .set("ButtonBackground", Color::transparent())
            .set(
                "ButtonBackgroundPointerOver",
                if close {
                    Color::rgb(196, 43, 64)
                } else {
                    Color::argb(35, 255, 255, 255)
                },
            )
            .set(
                "ButtonBackgroundPressed",
                if close {
                    Color::rgb(164, 34, 53)
                } else {
                    Color::argb(55, 255, 255, 255)
                },
            )
            .set("ButtonBorderBrush", Color::transparent())
            .set("ButtonBorderBrushPointerOver", Color::transparent())
            .set("ButtonBorderBrushPressed", Color::transparent())
            .set("ButtonForeground", TEXT)
            .set("ButtonForegroundPointerOver", TEXT)
            .set("ButtonForegroundPressed", TEXT)
            .set("ControlCornerRadius", CornerRadius::uniform(0.0));
        Button::new()
            .width(f64::from(CAPTION_BUTTONS_WIDTH) / 3.0)
            .height(f64::from(HEADER_HEIGHT))
            .automation_name(label)
            .resource_overrides(resources)
            .on_click(context.message(message))
            .content(FontIcon::new().glyph(glyph))
    });
    Grid::new()
        .columns([
            GridLength::STAR,
            GridLength::Pixel(f64::from(CAPTION_BUTTONS_WIDTH)),
        ])
        .children([
            drag,
            StackPanel::new()
                .grid_column(1)
                .orientation(Orientation::Horizontal)
                .children(buttons),
        ])
}
