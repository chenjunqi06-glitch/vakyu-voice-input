// AX 选区使用 UTF-16 偏移，不能使用 Swift 字符数量截断中文或 emoji。
import Foundation

struct SelectionSnapshot {
    let text: String?

    let range: CFRange?

    func unchanged(text current: String?, range currentRange: CFRange?) -> Bool {
        if let text, text != current { return false }
        if let range {
            guard let currentRange else { return false }
            if range.location != currentRange.location || range.length != currentRange.length { return false }
        }
        return true
    }

    func replacing(with insertion: String) -> String? {
        guard let text, let range else { return nil }
        let original = text as NSString
        guard range.location >= 0, range.length >= 0,
              range.location <= original.length,
              range.length <= original.length - range.location else { return nil }
        return original.replacingCharacters(in: NSRange(location: range.location, length: range.length), with: insertion)
    }
}
