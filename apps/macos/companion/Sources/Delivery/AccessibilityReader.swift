// AX 属性读取集中处理类型检查；没有可编辑目标时绝不向任意前台窗口投递。
import AppKit
import ApplicationServices
import Carbon

enum AccessibilityReader {
    static func value(_ element: AXUIElement, _ attribute: String) -> CFTypeRef? {
        var value: CFTypeRef?
        guard AXUIElementCopyAttributeValue(element, attribute as CFString, &value) == .success else { return nil }
        return value
    }

    static func text(_ element: AXUIElement, _ attribute: String) -> String? {
        value(element, attribute) as? String
    }

    static func element(_ source: AXUIElement, _ attribute: String) -> AXUIElement? {
        guard let value = value(source, attribute), CFGetTypeID(value) == AXUIElementGetTypeID() else { return nil }
        return (value as! AXUIElement)
    }

    static func editable(_ element: AXUIElement, _ attribute: String) -> Bool {
        var settable = DarwinBoolean(false)
        return AXUIElementIsAttributeSettable(element, attribute as CFString, &settable) == .success
            && settable.boolValue
    }

    static func range(_ element: AXUIElement) -> CFRange? {
        guard let value = value(element, kAXSelectedTextRangeAttribute), CFGetTypeID(value) == AXValueGetTypeID() else { return nil }
        let ax = value as! AXValue
        guard AXValueGetType(ax) == .cfRange else { return nil }
        var range = CFRange()
        guard AXValueGetValue(ax, .cfRange, &range) else { return nil }
        return range
    }

    static func secure(_ element: AXUIElement) -> Bool {
        let role = text(element, kAXRoleAttribute) ?? ""
        let subrole = text(element, kAXSubroleAttribute) ?? ""
        let protected = value(element, "AXProtectedContent") as? Bool ?? false
        return protected || role == "AXSecureTextField" || subrole == "AXSecureTextField"
    }

    static func focused(in processID: pid_t, logSource: Bool = false,
                        sourceObserved: (FocusResolution<AXUIElement>.Source) -> Void = { _ in }) -> FocusResolution<AXUIElement> {
        var available = true
        func read(_ source: AXUIElement, origin: String) -> AXUIElement? {
            AXUIElementSetMessagingTimeout(source, 0.25)
            var raw: CFTypeRef?
            let error = AXUIElementCopyAttributeValue(source, kAXFocusedUIElementAttribute as CFString, &raw)
            PermissionState.observeFocusError(error)
            if error == .apiDisabled || error == .cannotComplete { available = false }
            if logSource { NSLog("Vakyu input target focus read=%@ ax_error=%d", origin, error.rawValue) }
            guard error == .success, let raw, CFGetTypeID(raw) == AXUIElementGetTypeID() else { return nil }
            let focused = raw as! AXUIElement
            AXUIElementSetMessagingTimeout(focused, 0.25)
            return focused
        }
        return FocusResolution.resolve(processID: processID, currentProcessID: getpid(),
            frontmostPID: { NSWorkspace.shared.frontmostApplication?.processIdentifier },
            secureInput: { IsSecureEventInputEnabled() },
            systemFocus: { read(AXUIElementCreateSystemWide(), origin: "system") },
            applicationFocus: { read(AXUIElementCreateApplication($0), origin: "app") },
            nodePID: { element in
                var pid: pid_t = 0
                return AXUIElementGetPid(element, &pid) == .success ? pid : nil
            }, isSecure: secure, accessibilityAvailable: { available }, sourceObserved: { source in
                sourceObserved(source)
                if logSource { NSLog("Vakyu input target focus source=%@", source.rawValue) }
            })
    }

    static func webContent(_ source: AXUIElement) -> Bool {
        WebTargetPolicy.hasWebArea(from: source, role: { element in
            AXUIElementSetMessagingTimeout(element, 0.1)
            return text(element, kAXRoleAttribute)
        }, parent: { element($0, kAXParentAttribute) }, same: { CFEqual($0, $1) })
    }

    static func prepareFrontApplication(processID: pid_t) {
        guard processID != getpid(),
              NSWorkspace.shared.frontmostApplication?.processIdentifier == processID else { return }
        let app = AXUIElementCreateApplication(processID)
        // 超时必须在首次跨进程调用前设置，避免未响应的应用拖住按键处理。
        AXUIElementSetMessagingTimeout(app, 0.25)
        // Electron / Chromium 在需要辅助功能时才生成完整输入树；失败不修改其他属性。
        _ = AXUIElementSetAttributeValue(app, "AXManualAccessibility" as CFString, kCFBooleanTrue)
        _ = AXUIElementSetAttributeValue(app, "AXEnhancedUserInterface" as CFString, kCFBooleanTrue)
    }
}
