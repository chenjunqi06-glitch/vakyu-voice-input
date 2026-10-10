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

    @Published var keyboardPermission = false

    @Published var commitFailed = false

    @Published var entryMode = VoiceEntryMode.dictation

    var shortcutLabel: String {
        (try? Shortcut(configuration.shortcut).label) ?? "右 Control＋右 Option"
    }

    var dictationShortcutLabel: String {
        switch configuration.dictationKey {
        case "alt":
            if configuration.practice && (try? Shortcut(configuration.shortcut).rightControlOption) == true {
                return "左 Option"
            }
            return "左 / 右 Option"
        case "right_alt": return "右 Option"
        default: return "左 Option"
        }
    }

    var activeShortcutLabel: String { entryMode == .practice ? shortcutLabel : dictationShortcutLabel }

    func addLevel(_ level: Float) {
        meter.append(level)
    }

    func clearRound() {
        visible = false; recording = false; dictation = ""; error = ""
        practice = nil; commitFailed = false
        meter.reset()
    }

    func refreshPermissions() {
        microphonePermission = PermissionState.microphone
        accessibilityPermission = PermissionState.accessibility
        keyboardPermission = PermissionState.keyboard
    }
}
