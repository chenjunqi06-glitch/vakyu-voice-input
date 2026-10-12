//! 用完整草稿和 UTF-16 选区验证一次替换；只有确认毫无变化才可回退。
import Foundation

enum DeliveryVerification: Equatable {
    case confirmed

    case unchanged

    case uncertain

    static func inspect(before: SelectionSnapshot, insertion: String,
                        after: SelectionSnapshot) -> DeliveryVerification {
        guard let expected = before.replacing(with: insertion), let original = before.text,
              let current = after.text else { return .uncertain }
        if current == expected { return .confirmed }
        // replacing 已要求有效的原选区；当前选区也必须可读且与原选区相同。
        if current == original, after.range != nil,
           before.unchanged(text: after.text, range: after.range) { return .unchanged }
        return .uncertain
    }
}
