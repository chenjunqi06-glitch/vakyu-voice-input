//! 纯录入状态机；保留左右修饰键，组合键松开时不能退化成单独 Option。
import AppKit

struct ShortcutRecorder {
    private var pendingOption: UInt16?

    private var pendingChord: Shortcut?

    private var combinedModifiers = false

    mutating func update(down: Set<UInt16>, changed code: UInt16,
                         isDown: Bool, flags: CGEventFlags) throws -> Shortcut? {
        guard ShortcutKeyMap.isModifier(code) else {
            return isDown ? try Shortcut.fromEvent(code: code, flags: flags) : nil
        }
        if isDown {
            if down == [62, 61] || down == [54, 61] {
                pendingChord = down.contains(54) ? try Shortcut("right_win+right_alt") : .defaultValue
                pendingOption = nil
                combinedModifiers = true
                return nil
            }
            pendingChord = nil
            if (code == 58 || code == 61) && down == [code] && !combinedModifiers {
                pendingOption = code
            } else {
                pendingOption = nil
                combinedModifiers = true
            }
            return nil
        }
        if let shortcut = pendingChord, down.isEmpty {
            pendingChord = nil
            return shortcut
        }
        if pendingOption == code && down.isEmpty {
            pendingOption = nil
            return try Shortcut.fromEvent(code: code, flags: [])
        }
        if down.isEmpty && combinedModifiers {
            combinedModifiers = false
            throw NativeFailure("纯修饰键支持左 / 右 Option、右 Control＋右 Option 或右 Command＋右 Option。")
        }
        return nil
    }
}
