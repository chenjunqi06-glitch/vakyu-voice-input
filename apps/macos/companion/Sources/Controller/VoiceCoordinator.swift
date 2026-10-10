// 原生输入、音频和服务状态的唯一协调器；不在 UI 中重复实现学习逻辑。
import AppKit
import AVFoundation

final class VoiceCoordinator {
    let model = VoiceModel()

    let connection = ServiceConnection()

    let microphone = MicrophoneCapture()

    let speech = SpeechPlayer()

    let keyboard = KeyboardMonitor()

    let reservation = ShortcutReservation()

    let delivery = TextDelivery()

    lazy var overlay = OverlayPanel(model: model) { [weak self] action, index in
        self?.action(action, index: index)
    }

    lazy var settings = SettingsWindow(coordinator: self)

    lazy var statusItem = StatusItemController(coordinator: self)

    var activeGeneration: UInt64?

    var holdWork: DispatchWorkItem?

    var pressed = false

    var pressedMode: VoiceEntryMode?

    var pendingShortcut: (old: Shortcut, new: Shortcut)?

    var lastConfiguredShortcut: String?

    var roundEpoch: UInt64 = 0

    var discardUntilHide = false

    var serviceHealthy = true

    var restartRequested = false

    func start() {
        model.refreshPermissions()
        keyboard.onPress = { [weak self] mode in self?.keyPressed(mode: mode) }
        keyboard.onRelease = { [weak self] mode in self?.keyReleased(mode: mode) }
        keyboard.onInterrupted = { [weak self] in
            guard let self else { return }
            self.holdWork?.cancel(); self.holdWork = nil
            if self.pressed { self.cancel() }
        }
        keyboard.onAvailabilityChanged = { [weak self] available in
            guard let self else { return }
            if !available { self.reservation.release(); self.lastConfiguredShortcut = nil }
            else { self.synchronizeShortcut() }
        }
        keyboard.onEscape = { [weak self] in
            guard let self, self.model.visible else { return false }
            DispatchQueue.main.async { [weak self] in self?.cancel() }
            return true
        }
        keyboard.onRecorded = { [weak self] result in self?.recorded(result) }
        microphone.onAudio = { [weak connection] generation, data in
            connection?.sendAudio(generation: generation, data: data)
        }
        microphone.onLevel = { [weak model] level in model?.addLevel(level) }
        microphone.onFailure = { [weak self] generation, message in self?.captureFailed(message, generation: generation) }
        speech.onFinished = { [weak connection] id, ok in
            connection?.send(["command": "speech_finished", "id": id, "ok": ok])
        }
        connection.onEvent = { [weak self] event in self?.receive(event) }
        connection.onFailure = { [weak self] message in self?.serviceFailed(message) }
        connection.onStopped = { [weak self] in
            guard let self, self.restartRequested else { return }
            self.restartRequested = false
            self.discardUntilHide = false
            self.serviceHealthy = true
            do { try self.connection.launch() }
            catch { self.serviceFailed(error.localizedDescription) }
        }
        _ = keyboard.start()
        _ = statusItem
        do { try connection.launch() }
        catch { serviceFailed(error.localizedDescription) }
        if !model.microphonePermission || !model.accessibilityPermission || !keyboard.available {
            showSettings()
        }
    }

    func showSettings() { model.refreshPermissions(); settings.show() }

    func refreshPermissions() {
        model.refreshPermissions()
        if PermissionState.accessibility && PermissionState.keyboard { _ = keyboard.start() }
        else { keyboard.stop() }
        synchronizeShortcut()
    }

    func keyPressed(mode: VoiceEntryMode) {
        guard !model.visible, !discardUntilHide, model.configuration.enabled, !keyboard.recording else { return }
        guard mode != .practice || model.configuration.practice else { return }
        guard model.ready else { showError("语音模型还没有就绪，请在设置中查看状态。"); return }
        guard PermissionState.microphone else { showError("请先在语音设置中允许麦克风。"); showSettings(); return }
        do { delivery.reset(); delivery.target = try InputTarget.capture() }
        catch { showError(error.localizedDescription); return }
        holdWork?.cancel()
        let item = DispatchWorkItem { [weak self] in
            guard let self, self.keyboard.heldMode == mode else { return }
            self.roundEpoch &+= 1
            self.pressed = true
            self.pressedMode = mode
            self.model.clearRound()
            self.model.entryMode = mode
            self.model.visible = true
            self.model.status = "正在打开麦克风…"
            self.overlay.show(practice: false, lockScreen: true)
            self.connection.send(["command": "press", "mode": mode.rawValue])
        }
        holdWork = item
        DispatchQueue.main.asyncAfter(deadline: .now() + Double(model.configuration.holdMilliseconds) / 1000,
                                      execute: item)
    }

    func keyReleased(mode: VoiceEntryMode) {
        holdWork?.cancel(); holdWork = nil
        guard pressed, pressedMode == mode else { return }
        pressed = false
        pressedMode = nil
        let epoch = roundEpoch
        microphone.finish { [weak self] in
            guard let self, self.roundEpoch == epoch else { return }
            self.connection.send(["command": "release"])
        }
    }

    func cancel() {
        discardUntilHide = model.visible || pressed
        roundEpoch &+= 1
        holdWork?.cancel(); holdWork = nil; pressed = false; pressedMode = nil
        microphone.stop(); activeGeneration = nil; speech.cancel()
        connection.send(["command": "cancel"])
        delivery.reset(); model.clearRound(); overlay.hideRound()
    }

    func action(_ action: String, index: Int? = nil) {
        var command: [String: Any] = ["command": "action", "action": action]
        if let index { command["index"] = index }
        if action == "retry" || action == "restart" { model.error = "" }
        connection.send(command)
    }

    func captureFailed(_ message: String, generation failedGeneration: UInt64? = nil) {
        if let failedGeneration, failedGeneration != activeGeneration { return }
        microphone.stop(); model.recording = false
        if let generation = activeGeneration {
            connection.send(["command": "capture_failed", "generation": generation, "message": message])
        }
        activeGeneration = nil
        showError(message)
    }

    func serviceFailed(_ message: String) {
        guard serviceHealthy else { return }
        serviceHealthy = false
        discardUntilHide = false
        roundEpoch &+= 1; pressed = false; pressedMode = nil; holdWork?.cancel()
        microphone.stop(); activeGeneration = nil; speech.cancel()
        model.ready = false; model.recording = false; model.status = "语音服务未就绪"
        showError(message)
        connection.stop()
    }

    func showError(_ message: String) {
        model.error = message
        model.settingMessage = message
        if model.visible { overlay.show(practice: model.practice != nil) }
        else { statusItem.showNotice(message) }
    }

    func restart() {
        cancel()
        model.ready = false; model.status = "正在重新启动语音服务…"
        restartRequested = true
        connection.stop()
    }

    func shutdown() { restartRequested = false; cancel(); keyboard.stop(); reservation.release(); connection.stop() }
}
