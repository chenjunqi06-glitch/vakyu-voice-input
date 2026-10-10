// 主运行循环事件钩子，只做按键状态转换，不录音、不调用识别器。
import AppKit
import ApplicationServices

final class KeyboardMonitor {
    var onPress: ((VoiceEntryMode) -> Void)?

    var onRelease: ((VoiceEntryMode) -> Void)?

    var onEscape: (() -> Bool)?

    var onRecorded: ((Result<Shortcut, Error>) -> Void)?

    var onInterrupted: (() -> Void)?

    var onAvailabilityChanged: ((Bool) -> Void)?

    var shortcut: Shortcut { router.practiceShortcut }

    private var router = VoiceKeyRouter()

    var heldMode: VoiceEntryMode? { router.active }

    private(set) var recording = false

    private var pendingOption: UInt16?

    private var tap: CFMachPort?

    private var source: CFRunLoopSource?

    private var downKeys: Set<UInt16> = []

    private var pendingConfiguration: VoiceKeyConfiguration?

    var available: Bool { tap != nil }

    func configure(enabled: Bool, practiceEnabled: Bool, dictationKey: String, shortcut: Shortcut) {
        let configuration = VoiceKeyConfiguration(enabled: enabled, practiceEnabled: practiceEnabled,
                                                  dictationKey: dictationKey, shortcut: shortcut)
        if router.active != nil || !downKeys.isEmpty {
            pendingConfiguration = configuration
            return
        }
        pendingConfiguration = nil
        apply(configuration)
    }

    private func apply(_ configuration: VoiceKeyConfiguration) {
        let enabled = configuration.enabled
        let practiceEnabled = configuration.practiceEnabled
        let dictationKey = configuration.dictationKey
        let shortcut = configuration.shortcut
        guard router.enabled != enabled || router.practiceEnabled != practiceEnabled
            || router.dictationKey != dictationKey || router.practiceShortcut != shortcut else { return }
        router.enabled = enabled
        router.practiceEnabled = practiceEnabled
        router.dictationKey = dictationKey
        router.practiceShortcut = shortcut
        if downKeys.isEmpty { _ = router.update(down: [], changed: 0, isDown: false) }
    }

    @discardableResult
    func start() -> Bool {
        if tap != nil { return true }
        let mask = [CGEventType.keyDown, .keyUp, .flagsChanged].reduce(CGEventMask(0)) {
            $0 | CGEventMask(1 << $1.rawValue)
        }
        let callback: CGEventTapCallBack = { _, type, event, context in
            guard let context else { return Unmanaged.passUnretained(event) }
            let monitor = Unmanaged<KeyboardMonitor>.fromOpaque(context).takeUnretainedValue()
            return monitor.handle(type: type, event: event) ? nil : Unmanaged.passUnretained(event)
        }
        guard let newTap = CGEvent.tapCreate(tap: .cgSessionEventTap, place: .headInsertEventTap,
                                            options: .defaultTap, eventsOfInterest: mask,
                                            callback: callback,
                                            userInfo: Unmanaged.passUnretained(self).toOpaque()) else { return false }
        tap = newTap
        source = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, newTap, 0)
        if let source { CFRunLoopAddSource(CFRunLoopGetMain(), source, .commonModes) }
        CGEvent.tapEnable(tap: newTap, enable: true)
        onAvailabilityChanged?(true)
        return true
    }

    func stop() {
        if let mode = router.invalidate() { onRelease?(mode) }
        if let tap { CGEvent.tapEnable(tap: tap, enable: false); CFMachPortInvalidate(tap) }
        if let source { CFRunLoopRemoveSource(CFRunLoopGetMain(), source, .commonModes) }
        tap = nil; source = nil
        onAvailabilityChanged?(false)
    }

    func beginRecording() {
        if router.invalidate() != nil { onInterrupted?() }
        recording = true; pendingOption = nil
    }

    func endRecording() { recording = false; pendingOption = nil; _ = router.invalidate() }

    private func handle(type: CGEventType, event: CGEvent) -> Bool {
        if type == .tapDisabledByTimeout || type == .tapDisabledByUserInput {
            if let mode = router.invalidate() {
                DispatchQueue.main.async { [weak self] in self?.onRelease?(mode) }
            }
            onAvailabilityChanged?(false)
            if let tap {
                CGEvent.tapEnable(tap: tap, enable: true)
                onAvailabilityChanged?(CGEvent.tapIsEnabled(tap: tap))
            }
            return false
        }
        // 自己的粘贴按键不参与快捷键判定。
        if event.getIntegerValueField(.eventSourceUserData) == TextDelivery.eventMarker { return false }
        let code = UInt16(event.getIntegerValueField(.keyboardEventKeycode))
        let isDown = type == .keyDown || (type == .flagsChanged
            && CGEventSource.keyState(.combinedSessionState, key: code))
        downKeys = Set(downKeys.filter { CGEventSource.keyState(.combinedSessionState, key: $0) })
        if isDown && code != 57 { downKeys.insert(code) } else { downKeys.remove(code) }
        // 重新连接 tap 后也使用真实修饰键状态，防止前一个会话留下虚假的按下状态。
        for modifier in [UInt16(54), 55, 56, 58, 59, 60, 61, 62] {
            if CGEventSource.keyState(.combinedSessionState, key: modifier) { downKeys.insert(modifier) }
            else { downKeys.remove(modifier) }
        }
        if type == .keyDown && code == 53 {
            if recording { endRecording(); onRecorded?(.failure(NativeFailure("已取消录入。"))); return true }
            return onEscape?() ?? false
        }
        if recording { return record(type: type, event: event, code: code) }
        let previous = router.active
        let transition = router.update(down: downKeys, changed: code, isDown: isDown)
        if downKeys.isEmpty, let pending = pendingConfiguration {
            pendingConfiguration = nil
            apply(pending)
        }
        // AX 目标查询可能等待其他应用；回调出钩子后再执行，避免系统因超时卸载 tap。
        switch transition {
        case .pressed(let mode): DispatchQueue.main.async { [weak self] in self?.onPress?(mode) }
        case .released(let mode): DispatchQueue.main.async { [weak self] in self?.onRelease?(mode) }
        case .interrupted:
            DispatchQueue.main.async { [weak self] in self?.onInterrupted?() }
            return false
        case .none: break
        }
        return (router.active != nil || previous != nil)
            && (code == shortcut.keyCode || ShortcutKeyMap.isModifier(code))
    }

    private func record(type: CGEventType, event: CGEvent, code: UInt16) -> Bool {
        if type == .flagsChanged {
            if CGEventSource.keyState(.combinedSessionState, key: 62)
                && CGEventSource.keyState(.combinedSessionState, key: 61) {
                endRecording(); onRecorded?(.success(.defaultValue)); return true
            }
            if code == 58 || code == 61 {
                if CGEventSource.keyState(.combinedSessionState, key: code) { pendingOption = code }
                else if pendingOption == code {
                    let result = Result { try Shortcut.fromEvent(code: code, flags: []) }
                    endRecording(); onRecorded?(result)
                }
            }
            return true
        }
        if type == .keyDown {
            let result = Result { try Shortcut.fromEvent(code: code, flags: event.flags) }
            endRecording(); onRecorded?(result)
        }
        return true
    }
}
