//! 配置轻量监听：托盘切换后，已经打开的设置窗口也同步显示当前值。

use std::path::Path;
use std::time::Duration;

use windows_reactor::ComponentContext;

use super::{Message, Settings};

pub(super) fn watch(path: &Path, context: &ComponentContext<Settings>) {
    let path = path.to_owned();
    let modified = modified(&path);
    context.spawn_background(move |cancel| {
        while !cancel.is_cancelled() {
            std::thread::sleep(Duration::from_millis(400));
            if self::modified(&path) != modified {
                break;
            }
        }
        Message::ReloadConfig
    });
}

fn modified(path: &Path) -> Option<(std::time::SystemTime, u64)> {
    let metadata = std::fs::metadata(path).ok()?;
    Some((metadata.modified().ok()?, metadata.len()))
}
