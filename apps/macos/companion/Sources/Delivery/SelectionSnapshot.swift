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
        let selection = NSRange(location: range.location, length: range.length)
        // NSString 接受截断代理对的范围，必须额外拒绝 emoji 中间的无效边界。
        for offset in [range.location, range.location + range.length] where offset > 0 && offset < original.length {
            if (0xD800...0xDBFF).contains(original.character(at: offset - 1)),
               (0xDC00...0xDFFF).contains(original.character(at: offset)) { return nil }
        }
        return original.replacingCharacters(in: selection, with: insertion)
    }
}
