// 按下快捷键时绑定原输入框及选区，跟读期间切应用不会改变投递对象。
import AppKit
import ApplicationServices
import Carbon

struct InputTarget {
    let element: AXUIElement

    let window: AXUIElement?

    let processID: pid_t

    let originalValue: String?

    let originalRange: CFRange?

    let prefersPaste: Bool

    var fallbackWindowID: CGWindowID? = nil

    var fallbackHadFocus = false

    var usesPasteFallback: Bool { fallbackWindowID != nil }

    static func capture(allowPasteFallback: Bool = false) throws -> InputTarget {
        guard PermissionState.accessibility else {
            throw failure("accessibility_permission", "请先在 Vakyu 语音设置中允许“辅助功能”，才能填入当前输入框。")
        }
        guard !IsSecureEventInputEnabled() else {
            throw failure("secure_input", "系统正在使用安全输入，请离开密码框后再录音。")
        }
        guard let pid = NSWorkspace.shared.frontmostApplication?.processIdentifier, pid != getpid() else {
            throw failure("wrong_process", "请先点击你要填入文字的应用输入框。")
        }
        NSLog("Vakyu input target app=%@ pid=%d", NSRunningApplication(processIdentifier: pid)?.bundleIdentifier ?? "unknown", pid)
        let initialWindow = FrontWindow.identifier(processID: pid)
        AccessibilityReader.prepareFrontApplication(processID: pid)
        let element: AXUIElement
        var applicationFallback = false
        switch AccessibilityReader.focused(in: pid, logSource: true, sourceObserved: {
            applicationFallback = $0 == .application
        }) {
        case .element(let focused): element = focused
        case .missing:
            return try captureFallback(processID: pid, windowID: initialWindow, focus: nil,
                                       enabled: allowPasteFallback, reason: "no_focus")
        case .accessibilityUnavailable:
            throw failure("accessibility_unavailable", PermissionState.staleAccessibilityMessage)
        case .secureInput:
            throw failure("secure_input", "系统正在使用安全输入，请离开密码框后再录音。")
        case .secureElement:
            throw failure("secure_element", "密码框不支持语音输入，请先点击普通文本框。")
        case .contextChanged:
            throw failure("wrong_process", "请先点击你要填入文字的应用输入框。")
        }
        let role = AccessibilityReader.text(element, kAXRoleAttribute) ?? ""
        let supportedRole = ["AXTextField", "AXTextArea", "AXComboBox", "AXSearchField"].contains(role)
        let writable = AccessibilityReader.editable(element, kAXSelectedTextAttribute)
            || AccessibilityReader.editable(element, kAXValueAttribute)
        let enabled = AccessibilityReader.value(element, kAXEnabledAttribute) as? Bool ?? true
        guard enabled else {
            throw failure("unsupported_editor", "当前控件没有提供可编辑输入框，请先点正文或聊天框后重试。")
        }
        guard supportedRole || writable else {
            // 工具栏、菜单和按钮等明确非编辑控件不能因开关开启就变成粘贴目标。
            guard ["", "AXUnknown", "AXGroup", "AXWindow", "AXWebArea"].contains(role) else {
                throw failure("unsupported_editor", "当前控件不是文本输入框，请先点正文或聊天框后重试。")
            }
            return try captureFallback(processID: pid, windowID: initialWindow, focus: element,
                                       enabled: allowPasteFallback, reason: "unsupported_editor")
        }
        let target = InputTarget(element: element,
                                 window: AccessibilityReader.element(element, kAXWindowAttribute),
                                 processID: pid,
                                 originalValue: AccessibilityReader.text(element, kAXValueAttribute),
                                 originalRange: AccessibilityReader.range(element),
                                 prefersPaste: AccessibilityReader.webContent(element))
        if SelectionSnapshot(text: target.originalValue, range: target.originalRange).replacing(with: "") == nil {
            // 已返回但越界的选区仍属错误，不能借兼容路径绕过草稿保护。
            if target.originalValue != nil && target.originalRange != nil {
                throw failure("invalid_selection", "当前输入框选区无效，请重新点击文本框后重试。")
            }
            if applicationFallback || allowPasteFallback {
                return try captureFallback(processID: pid, windowID: initialWindow, focus: element,
                                           enabled: allowPasteFallback, reason: "unverifiable_editor")
            }
            throw failure("unverifiable_editor", "当前输入框未提供可核对的文字与选区。可在语音设置开启受限粘贴，或改用支持的文本框。")
        }
        guard !AccessibilityReader.secure(element) else {
            throw failure("secure_element", "密码框不支持语音输入，请先点击普通文本框。")
        }
        guard !IsSecureEventInputEnabled() else {
            throw failure("secure_input", "系统正在使用安全输入，请离开密码框后再录音。")
        }
        guard NSWorkspace.shared.frontmostApplication?.processIdentifier == pid else {
            throw failure("wrong_process", "请先点击你要填入文字的应用输入框。")
        }
        return target
    }

    private static func failure(_ reason: String, _ message: String) -> NativeFailure {
        NSLog("Vakyu input target rejected reason=%@", reason)
        return NativeFailure(message)
    }

    func validate(requireFocus: Bool) throws {
        try validateContext(requireFocus: requireFocus)
        if usesPasteFallback { return }
        guard requireFocus else { return }
        let snapshot = SelectionSnapshot(text: originalValue, range: originalRange)
        if !snapshot.unchanged(text: AccessibilityReader.text(element, kAXValueAttribute),
                               range: AccessibilityReader.range(element)) {
            throw NativeFailure("原输入框内容或选区已被修改，为保护草稿，本轮不会自动覆盖。请恢复原位置后重试，或按 Esc 重新录音。")
        }
    }

    func validateContext(requireFocus: Bool) throws {
        if usesPasteFallback { try validateFallback(); return }
        guard !IsSecureEventInputEnabled(), !AccessibilityReader.secure(element) else {
            throw NativeFailure("安全输入已开启，本轮不会填入密码或安全控件。")
        }
        guard let app = NSRunningApplication(processIdentifier: processID), !app.isTerminated,
              AccessibilityReader.text(element, kAXRoleAttribute) != nil,
              AccessibilityReader.value(element, kAXEnabledAttribute) as? Bool != false else {
            throw NativeFailure("原来的输入框已关闭，按 Esc 取消后请重新录音。")
        }
        guard requireFocus else { return }
        guard case .element(let current) = AccessibilityReader.focused(in: processID),
              CFEqual(current, element) else {
            throw NativeFailure("请回到录音开始时的输入框，再点“重新填入”；文字不会填到其他窗口。")
        }
        if let window {
            guard let currentWindow = AccessibilityReader.element(current, kAXWindowAttribute),
                  CFEqual(window, currentWindow) else {
                throw NativeFailure("无法确认仍在原窗口，请回到原输入框后重试。")
            }
        }
    }
}
