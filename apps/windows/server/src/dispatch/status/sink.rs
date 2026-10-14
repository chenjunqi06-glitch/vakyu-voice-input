use super::{StatusView, TrayState};

/// 状态条输出端。Router 在工人线程上调，窗口在 UI 线程上，故要 `Send`。
pub trait StatusSink: Send {
    /// 配置变化后刷新托盘菜单的勾选状态。
    fn configure_tray(&self, _state: TrayState) {}

    /// 开关划词翻译的「译」图标。
    fn configure_selection(&self, _enabled: bool) {}

    fn show_status(&self, view: StatusView);

    fn hide_status(&self);

    /// 起设置程序（任务栏图标右键菜单用；悬浮条上的齿轮在 UI 线程直接起）。
    fn open_settings(&self) {}

    /// 用浏览器打开下载页（右键菜单的「有新版本」）。
    fn open_download(&self) {}
}

/// 不画状态条的空实现。
pub struct NoopStatusSink;

impl StatusSink for NoopStatusSink {
    fn show_status(&self, _view: StatusView) {}

    fn hide_status(&self) {}
}
