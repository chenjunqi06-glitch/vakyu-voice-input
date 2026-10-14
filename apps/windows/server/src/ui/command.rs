//! 交给 UI 线程执行的命令。

use qingjian_platform::protocol::{Frame, ScreenRect};

use crate::dispatch::{PracticeView, RecordingMode, RenderSettings, StatusView, TrayState};

/// 交给 UI 线程执行的命令。`Frame` 较大，装箱免得枚举过胖。
pub(super) enum UiCommand {
    /// 把候选窗口摆到组句矩形下方并按帧重绘。
    Show(Box<(Frame, ScreenRect)>),

    /// 收起候选窗口。
    Hide,

    /// 全局语音按住状态的短租约，避免读取被钩子暂扣的 Alt 异步状态。
    VoiceHeld(bool),

    /// 当前麦克风的实测音量，零表示安静或已经停止采集。
    VoiceLevel(u8),

    /// 显示 / 更新按住 Alt 时的语音浮窗。字符串是当前实时识别结果，矩形是光标锚点。
    VoiceShow(Box<(String, ScreenRect, RecordingMode)>),

    /// 英语跟读：原文、逐词进度与录音状态。
    VoicePractice(Box<(PracticeView, ScreenRect)>),

    /// 尚在准备识别器，不提示用户已经开麦。
    VoicePrepare(ScreenRect, RecordingMode),

    /// 按住期间显示设备问题，松开即收起。
    VoiceError(Box<(String, ScreenRect, RecordingMode)>),

    /// 立即收起语音浮窗。
    VoiceHide,

    /// 显示 / 更新悬浮状态条。
    StatusShow(Box<StatusView>),

    /// 收起悬浮状态条。
    StatusHide,

    /// 更新托盘菜单上的开关勾选，不打断当前输入。
    TrayConfigure(TrayState),

    /// 换画法（渲染器 / 字体）。
    Configure(RenderSettings),

    /// 开关划词翻译的「译」图标。
    SelectionConfigure(bool),
}
