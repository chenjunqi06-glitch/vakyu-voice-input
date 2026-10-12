// 两入口互斥；任何一次触发后必须完全松开，残余修饰键不能开启第二轮。
struct VoiceKeyRouter {
    var dictationKey = "left_alt"

    var practiceShortcut = Shortcut.defaultValue

    var enabled = true

    var practiceEnabled = false

    private(set) var active: VoiceEntryMode?

    private var blockedUntilAllUp = false

    mutating func invalidate() -> VoiceEntryMode? {
        let old = active
        active = nil
        blockedUntilAllUp = true
        return old
    }

    mutating func update(down: Set<UInt16>, changed: UInt16, isDown: Bool) -> VoiceKeyTransition {
        if down.isEmpty {
            blockedUntilAllUp = false
            if let old = active { active = nil; return .released(old) }
            return .none
        }
        if blockedUntilAllUp { return .none }
        if let old = active {
            if matches(old, down: down) { return .none }
            active = nil
            blockedUntilAllUp = true
            return isDown ? .interrupted(old) : .released(old)
        }
        // 松开其他修饰键后留下 Option 不能被当作新的普通语音按下事件。
        guard enabled, isDown else { return .none }
        if practiceEnabled && matches(.practice, down: down) {
            active = .practice
            return .pressed(.practice)
        }
        if matches(.dictation, down: down) {
            active = .dictation
            return .pressed(.dictation)
        }
        return .none
    }

    private func matches(_ mode: VoiceEntryMode, down: Set<UInt16>) -> Bool {
        if mode == .dictation {
            switch dictationKey {
            case "alt":
                // 旧版“两侧 Option”在右侧组合跟读开启时，让出右 Option 前缀。
                if practiceEnabled && practiceShortcut.rightModifierOption { return down == [58] }
                return down == [58] || down == [61]
            case "right_alt": return down == [61]
            default: return down == [58]
            }
        }
        let shortcut = practiceShortcut
        if let keys = shortcut.modifierKeyCodes { return down == keys }
        guard down.contains(shortcut.keyCode) else { return false }
        var actual: UInt8 = 0
        for key in down where key != shortcut.keyCode {
            switch key {
            case 58, 61: actual |= 1
            case 59, 62: actual |= 2
            case 56, 60: actual |= 4
            case 54, 55: actual |= 8
            default: return false
            }
        }
        return actual == shortcut.modifiers
    }

    static func conflicts(dictationKey: String, practice: Shortcut) -> Bool {
        if practice.rightModifierOption { return dictationKey == "right_alt" }
        guard practice.modifierOnly else { return false }
        switch dictationKey {
        case "alt": return practice.keyCode == 58 || practice.keyCode == 61
        case "right_alt": return practice.keyCode == 61
        default: return practice.keyCode == 58
        }
    }
}
