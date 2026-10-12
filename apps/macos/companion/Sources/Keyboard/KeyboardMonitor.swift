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

    private var recorder = ShortcutRecorder()

    private let events: KeyboardEventSource

    private let keyState: (UInt16) -> Bool

    private var connectionEpoch: UInt64 = 0

    private var reportedAvailability = false

    private var downKeys: Set<UInt16> = []

    private var pendingConfiguration: VoiceKeyConfiguration?

    var available: Bool { events.available }

    init(events: KeyboardEventSource = SystemKeyboardEventSource(),
         keyState: @escaping (UInt16) -> Bool = { CGEventSource.keyState(.combinedSessionState, key: $0) }) {
        self.events = events
        self.keyState = keyState
    }

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
        if available { return true }
        connectionEpoch &+= 1
        interruptConnection()
        reportAvailability(false)
        let connected = events.start { [weak self] type, event in
            self?.handle(type: type, event: event) ?? false
        }
        // 停用期间可能错过全部松开事件，重连时按真实状态解除阻塞。
        downKeys = Set((UInt16(0)..<128).filter { $0 != 57 && keyState($0) })
        if downKeys.isEmpty {
            _ = router.update(down: [], changed: 0, isDown: false)
            applyPendingConfiguration()
        }
        reportAvailability(connected)
        return connected
    }

    func stop() {
        connectionEpoch &+= 1
        events.stop()
        interruptConnection()
        downKeys.removeAll()
        reportAvailability(false)
    }

    private func reportAvailability(_ value: Bool) {
        guard reportedAvailability != value else { return }
        reportedAvailability = value
        onAvailabilityChanged?(value)
    }

    private func interruptConnection() {
        if let mode = router.invalidate() { onRelease?(mode) }
        if recording {
            endRecording()
            // 断开监听后不确定是否漏掉按键，重连采样前仍须保持阻塞。
            _ = router.invalidate()
            onRecorded?(.failure(NativeFailure("键盘监听已中断，请刷新权限后重新录入快捷键。")))
        }
    }

    private func applyPendingConfiguration() {
        guard let pending = pendingConfiguration else { return }
        pendingConfiguration = nil
        apply(pending)
    }

    func beginRecording() {
        if router.invalidate() != nil { onInterrupted?() }
        recording = true; recorder = ShortcutRecorder()
    }

    func endRecording() {
        guard recording else { return }
        recording = false
        recorder = ShortcutRecorder()
        _ = router.invalidate()
        if downKeys.isEmpty {
            _ = router.update(down: [], changed: 0, isDown: false)
            applyPendingConfiguration()
        }
    }

    private func reconcileReleasedKeys(except code: UInt16) {
        let previous = downKeys
        // 只清理旧普通键；当次事件和修饰键快照不能被稍后的全局查询覆盖。
        downKeys = downKeys.filter {
            $0 == code || ShortcutKeyMap.isModifier($0) || $0 == 63 || keyState($0)
        }
        guard previous != downKeys, !recording else { return }
        // 丢失 keyUp 时仍须结束旧一轮；残余修饰键保持阻塞，全部松开才可重启。
        if case .released(let mode) = router.update(down: downKeys, changed: code, isDown: false) {
            DispatchQueue.main.async { [weak self] in self?.onRelease?(mode) }
        }
        if downKeys.isEmpty { applyPendingConfiguration() }
    }

    private func handle(type: CGEventType, event: CGEvent) -> Bool {
        if type == .tapDisabledByTimeout || type == .tapDisabledByUserInput {
            let epoch = connectionEpoch
            interruptConnection()
            reportAvailability(false)
            // 离开系统钩子回调后再重建，避免重入；显式停止后不得自动复活。
            DispatchQueue.main.async { [weak self] in
                guard let self, self.connectionEpoch == epoch else { return }
                _ = self.start()
            }
            return false
        }
        // 自己的粘贴按键不参与快捷键判定。
        if event.getIntegerValueField(.eventSourceUserData) == TextDelivery.eventMarker { return false }
        let code = UInt16(event.getIntegerValueField(.keyboardEventKeycode))
        reconcileReleasedKeys(except: code)
        if type == .keyDown && code != 57 { downKeys.insert(code) }
        else if type == .keyUp { downKeys.remove(code) }
        downKeys = ModifierKeyState.updating(downKeys, flags: event.flags, changed: code,
                                             flagsChanged: type == .flagsChanged, keyState: keyState)
        let isDown = type == .keyDown || (type == .flagsChanged && downKeys.contains(code))
        if type == .keyDown && code == 53 {
            if recording { endRecording(); onRecorded?(.failure(NativeFailure("已取消录入。"))); return true }
            return onEscape?() ?? false
        }
        if recording { return record(event: event, code: code, isDown: isDown) }
        let previous = router.active
        let transition = router.update(down: downKeys, changed: code, isDown: isDown)
        if downKeys.isEmpty { applyPendingConfiguration() }
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

    private func record(event: CGEvent, code: UInt16, isDown: Bool) -> Bool {
        do {
            if let shortcut = try recorder.update(down: downKeys, changed: code,
                                                  isDown: isDown, flags: event.flags) {
                endRecording(); onRecorded?(.success(shortcut))
            }
        } catch {
            endRecording(); onRecorded?(.failure(error))
        }
        return true
    }
}
