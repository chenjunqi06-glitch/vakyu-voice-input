use super::tray::TrayAction;
use crate::dispatch::PracticeAction;

/// 用户在状态条或托盘菜单上做的事，UI 线程发回 Router。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusEvent {
    /// 独立托盘菜单操作，不依赖前台应用加载输入法 DLL。
    Tray(TrayAction),

    /// 点了「中 / 英」格：翻转模式。
    ToggleMode,

    /// 点了「，。」格：翻转当前模式的全角标点（中英各记一份）。
    TogglePunctuation,

    /// 拖动结束，内容左上角的新位置（物理像素）。
    Moved(i32, i32),

    /// 英语跟读浮窗上的显式操作。
    Practice(PracticeAction),
}
