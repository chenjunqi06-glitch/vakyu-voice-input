//! 原生 AX 写入与受控粘贴；写入 API 返回值不作为文本已到达的证明。
import AppKit
import ApplicationServices

final class SystemTextDeliveryAccess: TextDeliveryAccess {
    func validate(_ target: InputTarget, protectDraft: Bool) throws {
        if protectDraft { try target.validate(requireFocus: true) }
        else { try target.validateContext(requireFocus: true) }
    }

    func snapshot(_ target: InputTarget) -> SelectionSnapshot {
        SelectionSnapshot(text: AccessibilityReader.text(target.element, kAXValueAttribute),
                          range: AccessibilityReader.range(target.element))
    }

    func editable(_ target: InputTarget, attribute: String) -> Bool {
        AccessibilityReader.editable(target.element, attribute)
    }

    func write(_ target: InputTarget, attribute: String, text: String) {
        _ = AXUIElementSetAttributeValue(target.element, attribute as CFString, text as CFString)
    }

    func moveCaret(_ target: InputTarget, to position: Int) {
        var range = CFRange(location: position, length: 0)
        if let value = AXValueCreate(.cfRange, &range) {
            _ = AXUIElementSetAttributeValue(target.element, kAXSelectedTextRangeAttribute as CFString, value)
        }
    }

    func paste(_ target: InputTarget, text: String) throws -> () -> Void {
        let board = NSPasteboard.general
        let saved = ClipboardSnapshot(board)
        board.clearContents()
        guard board.setString(text, forType: .string) else {
            saved.restore(board, ifUnchanged: board.changeCount)
            throw NativeFailure("无法写入临时剪贴板，请稍后重试。")
        }
        let written = board.changeCount
        let restore = { saved.restore(board, ifUnchanged: written) }
        do { try target.validate(requireFocus: true) }
        catch { restore(); throw error }
        guard let source = CGEventSource(stateID: .privateState),
              let down = CGEvent(keyboardEventSource: source, virtualKey: 9, keyDown: true),
              let up = CGEvent(keyboardEventSource: source, virtualKey: 9, keyDown: false) else {
            restore()
            throw NativeFailure("系统没有允许模拟粘贴，请检查辅助功能权限。")
        }
        guard board.changeCount == written else {
            throw NativeFailure("剪贴板已被其他操作修改，本轮未发送粘贴，请稍后重试。")
        }
        for event in [down, up] {
            event.flags = .maskCommand
            event.setIntegerValueField(.eventSourceUserData, value: TextDelivery.eventMarker)
            event.postToPid(target.processID)
        }
        return restore
    }
}
