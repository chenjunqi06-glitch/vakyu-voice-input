//! Vakyu 紫色渐变与随包图片；颜色与 Windows 品牌主题一致。

use objc2::AnyThread;
use objc2::rc::Retained;
use objc2_app_kit::{NSBezierPath, NSColor, NSGradient, NSImage};
use objc2_foundation::{NSRect, NSString};

pub fn image(name: &str) -> Option<Retained<NSImage>> {
    let path = super::paths::resource(name).ok()?;
    NSImage::initWithContentsOfFile(
        NSImage::alloc(),
        &NSString::from_str(&path.to_string_lossy()),
    )
}

pub fn background(rect: NSRect, radius: f64) {
    let start =
        NSColor::colorWithSRGBRed_green_blue_alpha(109.0 / 255.0, 53.0 / 255.0, 181.0 / 255.0, 1.0);
    let end =
        NSColor::colorWithSRGBRed_green_blue_alpha(53.0 / 255.0, 32.0 / 255.0, 99.0 / 255.0, 1.0);
    let path = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect, radius, radius);
    if let Some(gradient) =
        NSGradient::initWithStartingColor_endingColor(NSGradient::alloc(), &start, &end)
    {
        gradient.drawInBezierPath_angle(&path, -35.0);
    } else {
        start.set();
        path.fill();
    }
}
