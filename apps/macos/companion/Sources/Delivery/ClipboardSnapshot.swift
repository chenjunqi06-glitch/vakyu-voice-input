// 粘贴回退暂存所有剪贴板类型；用户期间复制了新内容时不覆盖它。
import AppKit

struct ClipboardSnapshot {
    let items: [[NSPasteboard.PasteboardType: Data]]

    init(_ board: NSPasteboard) {
        items = (board.pasteboardItems ?? []).map { item in
            var values: [NSPasteboard.PasteboardType: Data] = [:]
            for type in item.types { if let data = item.data(forType: type) { values[type] = data } }
            return values
        }
    }

    func restore(_ board: NSPasteboard, ifUnchanged changeCount: Int) {
        guard board.changeCount == changeCount else { return }
        let restored = items.map { values -> NSPasteboardItem in
            let item = NSPasteboardItem()
            for (type, data) in values { item.setData(data, forType: type) }
            return item
        }
        board.clearContents()
        if !restored.isEmpty { board.writeObjects(restored) }
    }
}
