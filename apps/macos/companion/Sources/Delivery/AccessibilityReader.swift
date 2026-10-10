// AX 属性读取集中处理类型检查；没有可编辑目标时绝不向任意前台窗口投递。
import AppKit
import ApplicationServices

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

    static func focused() -> AXUIElement? {
        element(AXUIElementCreateSystemWide(), kAXFocusedUIElementAttribute)
    }

    static func prepareFrontApplication() {
        guard let application = NSWorkspace.shared.frontmostApplication,
              application.processIdentifier != getpid() else { return }
        let app = AXUIElementCreateApplication(application.processIdentifier)
        // Electron / Chromium 在需要辅助功能时才生成完整输入树；失败不修改其他属性。
        _ = AXUIElementSetAttributeValue(app, "AXManualAccessibility" as CFString, kCFBooleanTrue)
        _ = AXUIElementSetAttributeValue(app, "AXEnhancedUserInterface" as CFString, kCFBooleanTrue)
        AXUIElementSetMessagingTimeout(app, 0.25)
    }
}
