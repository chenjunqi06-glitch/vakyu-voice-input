//! 不可读输入框的受限目标；始终核对原应用与窗口，已暴露的焦点也不得改变。
import AppKit
import ApplicationServices
import Carbon

extension InputTarget {
    static func captureFallback(processID: pid_t, windowID: CGWindowID?, focus: AXUIElement?,
                                enabled: Bool, reason: String) throws -> InputTarget {
        guard enabled else {
            NSLog("Vakyu input target rejected reason=%@ fallback=disabled", reason)
            throw NativeFailure("无法读取当前输入框。可在语音设置开启“无法读取输入框时直接粘贴”，或改用支持的文本框。")
        }
        let app = AXUIElementCreateApplication(processID)
        AXUIElementSetMessagingTimeout(app, 0.25)
        let window = focus.flatMap { AccessibilityReader.element($0, kAXWindowAttribute) }
            ?? AccessibilityReader.element(app, kAXFocusedWindowAttribute)
        guard let windowID else {
            NSLog("Vakyu input target rejected reason=unverifiable_window")
            throw NativeFailure("无法确认原应用窗口，本轮不会开始录音。请重新点击聊天输入框。")
        }
        let target = InputTarget(element: focus ?? app, window: window, processID: processID,
                                 originalValue: nil, originalRange: nil, prefersPaste: true,
                                 fallbackWindowID: windowID, fallbackHadFocus: focus != nil)
        try target.validateFallback()
        NSLog("Vakyu input target fallback reason=%@ window=%u", reason, windowID)
        return target
    }

    func validateFallback() throws {
        guard PermissionState.accessibility,
              let app = NSRunningApplication(processIdentifier: processID), !app.isTerminated else {
            throw NativeFailure("辅助功能不可用或原应用已关闭，本轮不会粘贴。")
        }
        let focus = AccessibilityReader.focused(in: processID)
        if case .element(let current) = focus {
            let role = AccessibilityReader.text(current, kAXRoleAttribute) ?? ""
            let permitted = ["", "AXUnknown", "AXGroup", "AXWindow", "AXWebArea",
                             "AXTextField", "AXTextArea", "AXComboBox", "AXSearchField"].contains(role)
                || AccessibilityReader.editable(current, kAXSelectedTextAttribute)
                || AccessibilityReader.editable(current, kAXValueAttribute)
            guard permitted, AccessibilityReader.value(current, kAXEnabledAttribute) as? Bool != false else {
                throw NativeFailure("当前焦点不是可输入位置，本轮未粘贴。")
            }
        }
        guard focus.permitsPasteFallback(enabled: true, processID: processID, savedWindow: fallbackWindowID,
            frontmostPID: { NSWorkspace.shared.frontmostApplication?.processIdentifier },
            currentWindow: { FrontWindow.identifier(processID: processID) },
            secureInput: { IsSecureEventInputEnabled() }) else {
            NSLog("Vakyu input target rejected reason=fallback_context_changed")
            throw NativeFailure("无法确认仍在原应用和原窗口，或安全输入已开启。本轮未粘贴，请复制文字后自行填入。")
        }
        if fallbackHadFocus {
            guard case .element(let current) = focus, CFEqual(current, element) else {
                throw NativeFailure("原输入位置已改变，本轮未粘贴。请复制文字后自行填入。")
            }
        }
        if let window, case .element(let current) = focus,
           let currentWindow = AccessibilityReader.element(current, kAXWindowAttribute),
           !CFEqual(window, currentWindow) {
            throw NativeFailure("原输入窗口已改变，本轮未粘贴。")
        }
        // 跨进程读取可能让出执行时间；发送前的最后检查不能使用旧的前台状态。
        guard NSWorkspace.shared.frontmostApplication?.processIdentifier == processID,
              FrontWindow.identifier(processID: processID) == fallbackWindowID,
              !IsSecureEventInputEnabled() else {
            throw NativeFailure("输入目标或安全状态已改变，本轮未粘贴。")
        }
    }
}
