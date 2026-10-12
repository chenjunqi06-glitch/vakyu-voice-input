// 只校验系统按键规则；语言与跟读策略仍由共享服务处理。
import AppKit
import Carbon

struct Shortcut: Equatable {
    let keyCode: UInt16

    let modifiers: UInt8

    let rightControlOption: Bool

    let canonical: String

    var rightCommandOption: Bool { canonical == "right_win+right_alt" }

    var rightModifierOption: Bool { rightControlOption || rightCommandOption }

    static let defaultValue = try! Shortcut("right_ctrl+right_alt")

    init(_ value: String) throws {
        var parts = value.lowercased().split(separator: "+").map {
            let part = $0.trimmingCharacters(in: .whitespaces)
            return ["right_cmd", "right_command"].contains(part) ? "right_win" : part
        }
        if Set(parts) == Set(["right_ctrl", "right_alt"]) && parts.count == 2 {
            keyCode = 61; modifiers = 2; rightControlOption = true
            canonical = "right_ctrl+right_alt"
            return
        }
        if Set(parts) == Set(["right_win", "right_alt"]) && parts.count == 2 {
            keyCode = 61; modifiers = 8; rightControlOption = false
            canonical = "right_win+right_alt"
            return
        }
        guard let key = parts.popLast(), let code = ShortcutKeyMap.codes[key] else {
            throw NativeFailure("请选择字母组合键、功能键或左 / 右 Option。")
        }
        var flags: UInt8 = 0
        for item in parts {
            switch item {
            case "alt", "option": flags |= 1
            case "ctrl", "control": flags |= 2
            case "shift": flags |= 4
            case "win", "cmd", "command": flags |= 8
            default: throw NativeFailure("无法识别这个修饰键。")
            }
        }
        let sideOption = key == "left_alt" || key == "right_alt"
        let function = key.count > 1 && key.first == "f"
        if sideOption && flags != 0 { throw NativeFailure("修饰键组合还需要一个字母或功能键。") }
        if !function && !sideOption && flags & 11 == 0 {
            throw NativeFailure("单独字母、数字、空格或 Shift＋字符会影响打字，请加入 Control、Option 或 Command。")
        }
        if key == "f12" || (key == "f4" && flags == 1) {
            throw NativeFailure("这个快捷键是跨平台保留键，请选择其他组合。")
        }
        if flags & 8 != 0 && ["q", "w", "h", "m", "space"].contains(key) {
            throw NativeFailure("这个组合会影响常用系统操作，请选择其他快捷键。")
        }
        keyCode = code; modifiers = flags; rightControlOption = false
        var names: [String] = []
        for (bit, name) in [(UInt8(2), "ctrl"), (4, "shift"), (1, "alt"), (8, "win")] {
            if flags & bit != 0 { names.append(name) }
        }
        names.append(key)
        canonical = names.joined(separator: "+")
    }

    var label: String {
        canonical.split(separator: "+").map {
            switch $0 {
            case "ctrl": return "Control"
            case "alt": return "Option"
            case "shift": return "Shift"
            case "win": return "Command"
            case "right_ctrl": return "右 Control"
            case "right_win": return "右 Command"
            case "right_alt": return "右 Option"
            case "left_alt": return "左 Option"
            case "space": return "空格"
            default: return $0.uppercased()
            }
        }.joined(separator: "＋")
    }

    var modifierOnly: Bool { modifierKeyCodes != nil }

    var modifierKeyCodes: Set<UInt16>? {
        if rightControlOption { return [62, 61] }
        if rightCommandOption { return [54, 61] }
        if keyCode == 58 || keyCode == 61 { return [keyCode] }
        return nil
    }

    var carbonModifiers: UInt32 {
        var value: UInt32 = 0
        if modifiers & 1 != 0 { value |= UInt32(optionKey) }
        if modifiers & 2 != 0 { value |= UInt32(controlKey) }
        if modifiers & 4 != 0 { value |= UInt32(shiftKey) }
        if modifiers & 8 != 0 { value |= UInt32(cmdKey) }
        return value
    }

    static func bits(_ flags: CGEventFlags) -> UInt8 {
        var value: UInt8 = 0
        if flags.contains(.maskAlternate) { value |= 1 }
        if flags.contains(.maskControl) { value |= 2 }
        if flags.contains(.maskShift) { value |= 4 }
        if flags.contains(.maskCommand) { value |= 8 }
        return value
    }

    func isHeld(flags: CGEventFlags) -> Bool {
        let down: (UInt16) -> Bool = { CGEventSource.keyState(.combinedSessionState, key: $0) }
        if let keys = modifierKeyCodes {
            return keys.allSatisfy(down) && Self.bits(flags) == (modifiers | 1)
        }
        return down(keyCode) && Self.bits(flags) == modifiers
    }

    static func fromEvent(code: UInt16, flags: CGEventFlags) throws -> Shortcut {
        guard let key = ShortcutKeyMap.name(code) else {
            throw NativeFailure("不支持这个按键，请选择功能键或字母组合。")
        }
        if code == 58 || code == 61 { return try Shortcut(key) }
        let bits = bits(flags)
        var names: [String] = []
        for (bit, name) in [(UInt8(2), "ctrl"), (4, "shift"), (1, "alt"), (8, "win")] {
            if bits & bit != 0 { names.append(name) }
        }
        names.append(key)
        return try Shortcut(names.joined(separator: "+"))
    }
}
