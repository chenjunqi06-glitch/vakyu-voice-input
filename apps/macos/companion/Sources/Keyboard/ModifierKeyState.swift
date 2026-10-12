//! 使用事件自身的修饰键快照，不能让全局会话查询覆盖已收到的松键事件。
import ApplicationServices

enum ModifierKeyState {
    // 左右位来自公开 IOKit/hidsystem/IOLLEvent.h 的 NX_DEVICE*KEYMASK。
    private static let groups: [(UInt16, UInt16, CGEventFlags, UInt64, UInt64)] = [
        (55, 54, .maskCommand, 0x08, 0x10),
        (56, 60, .maskShift, 0x02, 0x04),
        (58, 61, .maskAlternate, 0x20, 0x40),
        (59, 62, .maskControl, 0x01, 0x2000)
    ]

    static func updating(_ previous: Set<UInt16>, flags: CGEventFlags, changed code: UInt16,
                         flagsChanged: Bool, keyState: (UInt16) -> Bool) -> Set<UInt16> {
        var down = previous
        for (left, right, generic, leftMask, rightMask) in groups {
            down.remove(left); down.remove(right)
            guard flags.contains(generic) else { continue }
            if flags.rawValue & (leftMask | rightMask) != 0 {
                if flags.rawValue & leftMask != 0 { down.insert(left) }
                if flags.rawValue & rightMask != 0 { down.insert(right) }
            } else if flagsChanged && (code == left || code == right) {
                // 部分合成键盘事件只给通用位，此时按事件序列翻转发生变化的一侧。
                var held = previous.intersection([left, right])
                if held.contains(code) { held.remove(code) } else { held.insert(code) }
                if held.isEmpty { held.insert(code == left ? right : left) }
                down.formUnion(held)
            } else {
                let held = previous.intersection([left, right])
                if !held.isEmpty { down.formUnion(held) }
                else {
                    if keyState(left) { down.insert(left) }
                    if keyState(right) { down.insert(right) }
                    // 通用位仍有效但没有侧别时，至少保留修饰键，不能误触发单 Option。
                    if !down.contains(left) && !down.contains(right) { down.insert(left) }
                }
            }
        }
        // F 键和方向键也带 secondaryFn，只有 Fn 自身的变化才能确认物理 Fn。
        if flagsChanged && code == 63 {
            if flags.contains(.maskSecondaryFn) { down.insert(63) }
            else { down.remove(63) }
        }
        return down
    }
}
