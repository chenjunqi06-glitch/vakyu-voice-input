//! 内存 AX/粘贴替身与可控调度器；不读取系统控件、剪贴板或真实输入。
import ApplicationServices
@testable import VakyuVoice

final class TextDeliveryMemoryAccess: TextDeliveryAccess {
    var current: SelectionSnapshot

    var selectedWritable = true

    var valueWritable = true

    var contextValid = true

    var pastePreparationFails = false

    var writes: [String] = []

    var pastes = 0

    var restorations = 0

    var caretPositions: [Int] = []

    var pending: [() -> Void] = []

    var writeEffect: ((TextDeliveryMemoryAccess, String, String) -> Void)?

    var pasteEffect: ((TextDeliveryMemoryAccess, String) -> Void)?

    var validateEffect: ((TextDeliveryMemoryAccess, Bool) -> Void)?

    var snapshotEffect: ((TextDeliveryMemoryAccess) -> Void)?

    init(text: String? = "draft", range: CFRange? = CFRange(location: 5, length: 0)) {
        current = SelectionSnapshot(text: text, range: range)
    }

    func target(web: Bool = false) -> InputTarget {
        InputTarget(element: AXUIElementCreateApplication(999_999), window: nil, processID: 999_999,
                    originalValue: current.text, originalRange: current.range, prefersPaste: web)
    }

    func schedule(_ delay: TimeInterval, _ work: @escaping () -> Void) { pending.append(work) }

    func runNext() { if !pending.isEmpty { pending.removeFirst()() } }

    func runAll() { while !pending.isEmpty { runNext() } }

    func validate(_ target: InputTarget, protectDraft: Bool) throws {
        validateEffect?(self, protectDraft)
        guard contextValid else { throw NativeFailure("目标不再有效。") }
        let original = SelectionSnapshot(text: target.originalValue, range: target.originalRange)
        if protectDraft && !original.unchanged(text: current.text, range: current.range) {
            throw NativeFailure("草稿或选区已改变。")
        }
    }

    func snapshot(_ target: InputTarget) -> SelectionSnapshot { snapshotEffect?(self); return current }

    func editable(_ target: InputTarget, attribute: String) -> Bool {
        attribute == kAXSelectedTextAttribute ? selectedWritable : valueWritable
    }

    func write(_ target: InputTarget, attribute: String, text: String) {
        writes.append(attribute)
        if let writeEffect { writeEffect(self, attribute, text); return }
        if attribute == kAXSelectedTextAttribute { insert(text) }
        else { current = SelectionSnapshot(text: text, range: current.range) }
    }

    func moveCaret(_ target: InputTarget, to position: Int) {
        caretPositions.append(position)
        current = SelectionSnapshot(text: current.text, range: CFRange(location: position, length: 0))
    }

    func paste(_ target: InputTarget, text: String) throws -> () -> Void {
        guard !pastePreparationFails else { throw NativeFailure("发送前准备失败。") }
        pastes += 1
        if let pasteEffect { pasteEffect(self, text) } else { insert(text) }
        return { [weak self] in self?.restorations += 1 }
    }

    func insert(_ text: String) {
        let next = current.range.map { CFRange(location: $0.location + (text as NSString).length, length: 0) }
        current = SelectionSnapshot(text: current.replacing(with: text), range: next)
    }
}
