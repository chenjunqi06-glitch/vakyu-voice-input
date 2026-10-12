// UI 的轻量状态；音量最多每秒更新 30 次，未录音时没有动画定时器。
import SwiftUI

final class VoiceModel: ObservableObject {
    @Published var configuration = VoiceConfiguration()

    @Published var ready = false

    @Published var visible = false

    @Published var recording = false

    @Published var dictation = ""

    @Published var status = "语音模型准备中…"

    @Published var error = ""

    @Published var practice: PracticeSnapshot?

    let meter = AudioMeterState()

    @Published var shortcutRecording = false

    @Published var shortcutMessage = ""

    @Published var settingMessage = ""

    @Published var microphonePermission = false

    @Published var accessibilityPermission = false

    @Published var accessibilityMayBeStale = false

    @Published var keyboardPermission = false

    @Published var keyboardMonitorAvailable = false

    @Published var commitFailed = false

    @Published var pastedWithoutVerification = false

    /// 仅保留在内存中，供用户主动复制；不写入配置或日志。
    var lastResult = ""

    @Published var entryMode = VoiceEntryMode.dictation

    var shortcutLabel: String {
        (try? Shortcut(configuration.shortcut).label) ?? "右 Control＋右 Option"
    }

    var dictationShortcutLabel: String {
        switch configuration.dictationKey {
        case "alt":
            if configuration.practice && (try? Shortcut(configuration.shortcut).rightModifierOption) == true {
                return "左 Option"
            }
            return "左 / 右 Option"
        case "right_alt": return "右 Option"
        default: return "左 Option"
        }
    }

    var activeShortcutLabel: String { entryMode == .practice ? shortcutLabel : dictationShortcutLabel }

    var availabilityMessage: String {
        if !configuration.enabled { return "全局语音输入已关闭" }
        if !accessibilityPermission { return "快捷键不可用：请允许辅助功能" }
        if accessibilityMayBeStale { return PermissionState.staleAccessibilityMessage }
        if !keyboardMonitorAvailable { return "快捷键尚未连接，请在系统权限中重新检查" }
        if !microphonePermission { return "录音不可用：请允许麦克风" }
        return ready ? "语音与快捷键已就绪" : status
    }

    var permissionsReady: Bool {
        microphonePermission && accessibilityPermission && keyboardMonitorAvailable && !accessibilityMayBeStale
    }

    var inputReady: Bool { configuration.enabled && ready && permissionsReady }

    func addLevel(_ level: Float) {
        meter.append(level)
    }

    func clearRound() {
        visible = false; recording = false; dictation = ""; error = ""
        practice = nil; commitFailed = false; pastedWithoutVerification = false
        meter.reset()
    }

    func refreshPermissions() {
        let microphone = PermissionState.microphone
        let accessibility = PermissionState.accessibility
        let keyboard = PermissionState.keyboard
        if microphonePermission != microphone { microphonePermission = microphone }
        if accessibilityPermission != accessibility { accessibilityPermission = accessibility }
        if accessibilityMayBeStale != PermissionState.accessibilityMayBeStale {
            accessibilityMayBeStale = PermissionState.accessibilityMayBeStale
        }
        if keyboardPermission != keyboard { keyboardPermission = keyboard }
    }
}
