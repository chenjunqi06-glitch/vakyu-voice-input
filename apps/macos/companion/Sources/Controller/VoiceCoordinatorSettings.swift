// 设置先由原生层注册快捷键，再请求共享服务持久化；失败恢复原键。
import AppKit

extension VoiceCoordinator {
    func set(_ key: String, value: Any) {
        connection.send(["command": "set", "key": key, "value": value])
    }

    func recordShortcut() {
        guard keyboard.start() else {
            model.shortcutMessage = "请先允许辅助功能与输入监控，然后点“刷新权限”。"
            return
        }
        keyboard.beginRecording(); model.shortcutRecording = true
        model.shortcutMessage = "按下新的口语跟读快捷键；不能与普通语音键相同，Esc 取消。"
    }

    func recorded(_ result: Result<Shortcut, Error>) {
        model.shortcutRecording = false
        switch result {
        case .success(let shortcut): saveShortcut(shortcut)
        case .failure(let error): model.shortcutMessage = error.localizedDescription
        }
    }

    func saveShortcut(_ shortcut: Shortcut) {
        guard pendingShortcut == nil else { return }
        guard !VoiceKeyRouter.conflicts(dictationKey: model.configuration.dictationKey, practice: shortcut) else {
            model.shortcutMessage = "该快捷键与普通语音键重叠，请为口语跟读选择其他快捷键。原设置保持不变。"
            return
        }
        do {
            let previous = keyboard.shortcut
            try reservation.replace(with: shortcut)
            pendingShortcut = (previous, shortcut)
            model.shortcutMessage = "正在保存快捷键…"
            set("voice_practice_shortcut", value: shortcut.canonical)
        } catch { model.shortcutMessage = error.localizedDescription }
    }

    func settingResult(_ object: [String: Any]) {
        let success = object["ok"] as? Bool ?? false
        let message = object["message"] as? String ?? (success ? "已保存" : "设置保存失败。")
        model.settingMessage = message
        if object["key"] as? String == "voice_practice_shortcut", let pending = pendingShortcut {
            pendingShortcut = nil
            if success {
                model.configuration.shortcut = pending.new.canonical
                lastConfiguredShortcut = nil
                model.shortcutMessage = pending.new.modifierOnly
                    ? "已保存。纯修饰键不能检测占用；应用内部快捷键也可能无法检测。" : "快捷键已保存，立即生效。"
                if !model.configuration.enabled || !model.configuration.practice || !keyboard.available {
                    reservation.release(); lastConfiguredShortcut = nil
                }
            } else {
                do { try reservation.replace(with: pending.old) }
                catch {
                    keyboard.configure(enabled: model.configuration.enabled, practiceEnabled: false,
                                       dictationKey: model.configuration.dictationKey, shortcut: pending.old)
                    model.shortcutMessage = "原跟读快捷键已被其他程序占用，请重新选择。普通语音仍可用。"
                    return
                }
                model.configuration.shortcut = pending.old.canonical
                model.shortcutMessage = message
            }
            synchronizeShortcut()
        }
        statusItem.refresh()
    }

    func openInputSettings() {
        var parent = Bundle.main.bundleURL.deletingLastPathComponent()
        while parent.path != "/" && parent.pathExtension != "app" { parent.deleteLastPathComponent() }
        if parent.pathExtension == "app" {
            let configuration = NSWorkspace.OpenConfiguration()
            configuration.arguments = ["--preferences"]
            NSWorkspace.shared.openApplication(at: parent, configuration: configuration) { _, _ in
                DistributedNotificationCenter.default().postNotificationName(
                    NSNotification.Name("app.vakyu.inputmethod.open-settings"), object: nil,
                    userInfo: nil, deliverImmediately: true)
            }
        } else {
            model.settingMessage = "找不到输入法主程序，请从输入菜单打开 Vakyu 设置。"
        }
    }
}
