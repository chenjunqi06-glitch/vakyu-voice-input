// 优先 AX 选区插入，回退受控粘贴；从不生成 Enter 或发送快捷键。
import AppKit
import ApplicationServices

final class TextDelivery {
    static let eventMarker: Int64 = 0x56414B5955

    var target: InputTarget?

    private var completed: Set<UInt64> = []

    private var uncertain = false

    private var epoch: UInt64 = 0

    func reset() { epoch &+= 1; target = nil; completed.removeAll(); uncertain = false }

    func commit(id: UInt64, text: String, completion: @escaping (Bool, String) -> Void) {
        if completed.contains(id) { completion(true, ""); return }
        if uncertain {
            completion(false, "已尝试一次粘贴，但该控件无法确认结果。请检查原输入框；为避免重复，不会再次粘贴。")
            return
        }
        guard let target else { completion(false, "没有保存输入目标，请按 Esc 后重新录音。"); return }
        do { try target.validate(requireFocus: true) }
        catch { completion(false, error.localizedDescription); return }
        guard !text.isEmpty else { completion(false, "没有可填入的识别文字。"); return }
        let element = target.element
        if AccessibilityReader.editable(element, kAXSelectedTextAttribute),
           AXUIElementSetAttributeValue(element, kAXSelectedTextAttribute as CFString, text as CFString) == .success {
            completed.insert(id); completion(true, ""); return
        }
        if let old = target.originalValue, let selected = target.originalRange,
           AccessibilityReader.editable(element, kAXValueAttribute) {
            let original = old as NSString
            if selected.location >= 0 && selected.length >= 0
                && selected.location <= original.length && selected.length <= original.length - selected.location {
                let range = NSRange(location: selected.location, length: selected.length)
                let result = original.replacingCharacters(in: range, with: text)
                if AXUIElementSetAttributeValue(element, kAXValueAttribute as CFString, result as CFString) == .success {
                    var next = CFRange(location: selected.location + (text as NSString).length, length: 0)
                    if let value = AXValueCreate(.cfRange, &next) {
                        _ = AXUIElementSetAttributeValue(element, kAXSelectedTextRangeAttribute as CFString, value)
                    }
                    completed.insert(id); completion(true, ""); return
                }
            }
        }
        paste(id: id, text: text, target: target, completion: completion)
    }

    private func paste(id: UInt64, text: String, target: InputTarget,
                       completion: @escaping (Bool, String) -> Void) {
        let board = NSPasteboard.general
        let snapshot = ClipboardSnapshot(board)
        board.clearContents()
        guard board.setString(text, forType: .string) else {
            snapshot.restore(board, ifUnchanged: board.changeCount)
            completion(false, "无法写入临时剪贴板，请稍后重试。")
            return
        }
        let written = board.changeCount
        do { try target.validate(requireFocus: true) }
        catch {
            snapshot.restore(board, ifUnchanged: written)
            completion(false, error.localizedDescription)
            return
        }
        guard let source = CGEventSource(stateID: .privateState),
              let down = CGEvent(keyboardEventSource: source, virtualKey: 9, keyDown: true),
              let up = CGEvent(keyboardEventSource: source, virtualKey: 9, keyDown: false) else {
            snapshot.restore(board, ifUnchanged: written)
            completion(false, "系统没有允许模拟粘贴，请检查辅助功能权限。")
            return
        }
        uncertain = true
        let deliveryEpoch = epoch
        for event in [down, up] {
            event.flags = .maskCommand
            event.setIntegerValueField(.eventSourceUserData, value: Self.eventMarker)
            event.postToPid(target.processID)
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.35) { [weak self] in
            guard let self, self.epoch == deliveryEpoch else {
                snapshot.restore(board, ifUnchanged: written)
                return
            }
            let after = AccessibilityReader.text(target.element, kAXValueAttribute)
            var confirmed = false
            if let before = target.originalValue, let range = target.originalRange, let after {
                let string = before as NSString
                if range.location >= 0 && range.length >= 0 && range.location <= string.length
                    && range.length <= string.length - range.location {
                    let expected = string.replacingCharacters(in: NSRange(location: range.location, length: range.length), with: text)
                    confirmed = after == expected
                }
            } else if let after, after != target.originalValue { confirmed = after.contains(text) }
            // 不可读取的输入框不能把“发出了按键”当作确认成功。
            if confirmed {
                self.uncertain = false; self.completed.insert(id); completion(true, "")
            } else {
                completion(false, "已尝试粘贴，但该控件没有返回可验证的结果。请检查原输入框；为避免重复，不会自动再次粘贴。")
            }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.4) {
                snapshot.restore(board, ifUnchanged: written)
            }
        }
    }
}
