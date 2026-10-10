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

    static func capture() throws -> InputTarget {
        guard PermissionState.accessibility else {
            throw NativeFailure("请先在 Vakyu 语音设置中允许“辅助功能”，才能填入当前输入框。")
        }
        guard !IsSecureEventInputEnabled() else {
            throw NativeFailure("系统正在使用安全输入，请离开密码框后再录音。")
        }
        AccessibilityReader.prepareFrontApplication()
        guard let element = AccessibilityReader.focused(), !AccessibilityReader.secure(element) else {
            throw NativeFailure("请先点击可编辑的输入框；密码框不支持语音输入。")
        }
        AXUIElementSetMessagingTimeout(element, 0.25)
        var pid: pid_t = 0
        guard AXUIElementGetPid(element, &pid) == .success, pid != getpid(),
              NSWorkspace.shared.frontmostApplication?.processIdentifier == pid else {
            throw NativeFailure("请先点击你要填入文字的应用输入框。")
        }
        let role = AccessibilityReader.text(element, kAXRoleAttribute) ?? ""
        let supportedRole = ["AXTextField", "AXTextArea", "AXComboBox", "AXSearchField"].contains(role)
        let writable = AccessibilityReader.editable(element, kAXSelectedTextAttribute)
            || AccessibilityReader.editable(element, kAXValueAttribute)
        let enabled = AccessibilityReader.value(element, kAXEnabledAttribute) as? Bool ?? true
        guard enabled && (supportedRole || writable) else {
            throw NativeFailure("当前控件没有提供可编辑输入框，请先点正文或聊天框后重试。")
        }
        return InputTarget(element: element,
                           window: AccessibilityReader.element(element, kAXWindowAttribute),
                           processID: pid,
                           originalValue: AccessibilityReader.text(element, kAXValueAttribute),
                           originalRange: AccessibilityReader.range(element))
    }

    func validate(requireFocus: Bool) throws {
        guard !IsSecureEventInputEnabled(), !AccessibilityReader.secure(element) else {
            throw NativeFailure("安全输入已开启，本轮不会填入密码或安全控件。")
        }
        guard let app = NSRunningApplication(processIdentifier: processID), !app.isTerminated,
              AccessibilityReader.text(element, kAXRoleAttribute) != nil,
              AccessibilityReader.value(element, kAXEnabledAttribute) as? Bool != false else {
            throw NativeFailure("原来的输入框已关闭，按 Esc 取消后请重新录音。")
        }
        guard requireFocus else { return }
        guard NSWorkspace.shared.frontmostApplication?.processIdentifier == processID,
              let current = AccessibilityReader.focused(), CFEqual(current, element) else {
            throw NativeFailure("请回到录音开始时的输入框，再点“重新填入”；文字不会填到其他窗口。")
        }
        if let window,
           let currentWindow = AccessibilityReader.element(current, kAXWindowAttribute),
           !CFEqual(window, currentWindow) {
            throw NativeFailure("当前窗口已改变，请回到原输入框后重试。")
        }
        let snapshot = SelectionSnapshot(text: originalValue, range: originalRange)
        if !snapshot.unchanged(text: AccessibilityReader.text(current, kAXValueAttribute),
                               range: AccessibilityReader.range(current)) {
            throw NativeFailure("原输入框内容或选区已被修改，为保护草稿，本轮不会自动覆盖。请恢复原位置后重试，或按 Esc 重新录音。")
        }
    }
}
