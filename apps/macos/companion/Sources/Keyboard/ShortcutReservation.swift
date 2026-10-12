// 保存前先尝试系统注册，成功后替换旧键；纯修饰键无法使用 Carbon 检测占用。
import Carbon

final class ShortcutReservation {
    private var reference: EventHotKeyRef?

    private var current: Shortcut?

    private var nextID: UInt32 = 1

    func replace(with shortcut: Shortcut) throws {
        if current == shortcut { return }
        var candidate: EventHotKeyRef?
        if !shortcut.modifierOnly {
            let identity = EventHotKeyID(signature: OSType(0x56414B59), id: nextID)
            nextID &+= 1
            let status = RegisterEventHotKey(UInt32(shortcut.keyCode), shortcut.carbonModifiers,
                                             identity, GetApplicationEventTarget(), 0, &candidate)
            guard status == noErr, candidate != nil else {
                throw NativeFailure("该快捷键产生冲突，请更换。原来的快捷键仍然保留。")
            }
        }
        if let reference { UnregisterEventHotKey(reference) }
        reference = candidate
        current = shortcut
    }

    func release() {
        if let reference { UnregisterEventHotKey(reference) }
        reference = nil; current = nil
    }

    deinit { release() }
}
