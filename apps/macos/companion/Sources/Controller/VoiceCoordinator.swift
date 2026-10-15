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

    let completionPresentation: CompletionPresentation

    var queuedPress: VoiceEntryMode?

    lazy var overlay = OverlayPanel(model: model) { [weak self] action, index in
        self?.action(action, index: index)
    }

    lazy var settings = SettingsWindow(coordinator: self)

    lazy var statusItem = StatusItemController(coordinator: self)

    lazy var quickBar = QuickBarPanel(model: model) { [weak self] sendTranslation in
        self?.togglePracticeOutput(sendTranslation: sendTranslation)
    }

    lazy var translationPanel = TranslationPanel()

    let translationService = TranslationService()

    let systemTranslator = SystemTranslator()

    var translationRequest: UInt64 = 0

    var activeGeneration: UInt64?

    var holdWork: DispatchWorkItem?

    var pressed = false

    var pressedMode: VoiceEntryMode?

    var pendingShortcut: (old: Shortcut, new: Shortcut)?

    var lastConfiguredShortcut: String?

    var roundEpoch: UInt64 = 0

    var triggerSequence: UInt64 = 0

    var discardUntilHide = false

    var serviceHealthy = true

    var restartRequested = false

    /// 服务意外退出后的自动重启时间点；短时间内反复失败就停下，交给用户在设置里手动重启。
    var automaticRestarts: [Date] = []

    var automaticRestartPending = false

    var shuttingDown = false

    private var lastRuntimeState = ""

    init(completionPresentation: CompletionPresentation = CompletionPresentation()) {
        self.completionPresentation = completionPresentation
    }

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
            self.model.keyboardMonitorAvailable = available
            if !available { self.reservation.release(); self.lastConfiguredShortcut = nil }
            else { self.synchronizeShortcut() }
            self.refreshRuntimeStatus()
        }
        keyboard.onEscape = { [weak self] in
            guard let self, self.model.visible, !self.completionPresentation.collapsed else { return false }
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
            // 自动恢复稍等片刻，让崩溃的 Python 子进程和麦克风先释放；手动重启立即进行。
            let delay: TimeInterval = self.automaticRestartPending ? 2 : 0
            self.automaticRestartPending = false
            DispatchQueue.main.asyncAfter(deadline: .now() + delay) { [weak self] in
                guard let self, !self.shuttingDown else { return }
                self.discardUntilHide = false
                self.serviceHealthy = true
                do { try self.connection.launch() }
                catch { self.serviceFailed(error.localizedDescription) }
            }
        }
        refreshPermissions()
        _ = statusItem
        quickBar.restore()
        translationService.onText = { [weak self] text in self?.translate(text) }
        translationService.register()
        do { try connection.launch() }
        catch { serviceFailed(error.localizedDescription) }
        if !model.microphonePermission || !model.accessibilityPermission || !keyboard.available {
            showSettings()
        }
    }

    func showSettings() { refreshPermissions(); settings.show() }

    func refreshPermissions() {
        model.refreshPermissions()
        // 活跃事件钩子由辅助功能授权；以系统实际创建结果为准，不能用被动监听预检挡住它。
        if model.accessibilityPermission { _ = keyboard.start() }
        else { keyboard.stop() }
        if model.keyboardMonitorAvailable != keyboard.available { model.keyboardMonitorAvailable = keyboard.available }
        synchronizeShortcut()
        refreshRuntimeStatus()
    }

    func refreshRuntimeStatus() {
        // 只记录状态变化，不记录按键、音频、原文或服务凭据。
        let state = "mic=\(model.microphonePermission) ax=\(model.accessibilityPermission) "
            + "input=\(model.keyboardPermission) tap=\(keyboard.available) ready=\(model.ready) "
            + "enabled=\(model.configuration.enabled) practice=\(model.configuration.practice)"
        guard state != lastRuntimeState else { return }
        lastRuntimeState = state
        NSLog("Vakyu voice status: %@", state)
        statusItem.refresh()
    }

    func keyPressed(mode: VoiceEntryMode) {
        triggerSequence &+= 1
        let trigger = triggerSequence
        // 只记录入口、固定原因码和耗时，不记录应用内容、录音、按键序列或凭据。
        NSLog("Vakyu voice trigger %llu: matched mode=%@", trigger, mode.rawValue)
        // 失败结果仍可从菜单恢复；开启新一轮前先等服务取消旧会话，避免旧 Hide 关掉新窗。
        if model.visible && model.commitFailed {
            cancel()
            queuedPress = mode
            return
        }
        let blocked: String?
        if model.visible { blocked = "round_visible" }
        else if discardUntilHide { blocked = "cancel_pending" }
        else if !model.configuration.enabled { blocked = "disabled" }
        else if keyboard.recording { blocked = "shortcut_recording" }
        else if mode == .practice && !model.configuration.practice { blocked = "practice_disabled" }
        else { blocked = nil }
        if let blocked {
            NSLog("Vakyu voice trigger %llu: ignored reason=%@", trigger, blocked)
            return
        }
        guard model.ready else {
            NSLog("Vakyu voice trigger %llu: rejected reason=model_not_ready", trigger)
            showError("语音模型还没有就绪，请在设置中查看状态。"); return
        }
        guard PermissionState.microphone else {
            NSLog("Vakyu voice trigger %llu: rejected reason=microphone_permission", trigger)
            showError("请先在语音设置中允许麦克风。"); showSettings(); return
        }
        let captureStarted = ProcessInfo.processInfo.systemUptime
        do {
            delivery.reset()
            delivery.target = try InputTarget.capture(allowPasteFallback: model.configuration.pasteFallback)
        }
        catch {
            model.refreshPermissions()
            let elapsed = (ProcessInfo.processInfo.systemUptime - captureStarted) * 1000
            NSLog("Vakyu voice trigger %llu: rejected reason=input_target elapsed=%.0f ms", trigger, elapsed)
            showError(error.localizedDescription); return
        }
        let elapsed = (ProcessInfo.processInfo.systemUptime - captureStarted) * 1000
        NSLog("Vakyu voice trigger %llu: input target ready in %.0f ms", trigger, elapsed)
        holdWork?.cancel()
        let item = DispatchWorkItem { [weak self] in
            guard let self else { return }
            self.holdWork = nil
            guard self.keyboard.heldMode == mode else {
                NSLog("Vakyu voice trigger %llu: released before hold threshold", trigger)
                return
            }
            self.roundEpoch &+= 1
            self.completionPresentation.reset()
            self.pressed = true
            self.pressedMode = mode
            self.model.clearRound()
            self.model.entryMode = mode
            self.model.visible = true
            self.model.status = "正在打开麦克风…"
            self.overlay.show(practice: false, lockScreen: true)
            NSLog("Vakyu voice trigger %llu: started round %llu", trigger, self.roundEpoch)
            self.connection.send(["command": "press", "mode": mode.rawValue])
        }
        holdWork = item
        DispatchQueue.main.asyncAfter(deadline: .now() + Double(model.configuration.holdMilliseconds) / 1000,
                                      execute: item)
    }

    func keyReleased(mode: VoiceEntryMode) {
        if queuedPress == mode { queuedPress = nil }
        if holdWork != nil && !pressed {
            NSLog("Vakyu voice trigger %llu: released before hold threshold", triggerSequence)
        }
        holdWork?.cancel(); holdWork = nil
        guard pressed, pressedMode == mode else { return }
        pressed = false
        pressedMode = nil
        model.recording = false
        model.status = "正在处理…"
        model.meter.reset()
        let epoch = roundEpoch
        let releasedAt = ProcessInfo.processInfo.systemUptime
        NSLog("Vakyu voice round %llu: shortcut released", epoch)
        microphone.finish { [weak self] in
            guard let self, self.roundEpoch == epoch else { return }
            let elapsed = (ProcessInfo.processInfo.systemUptime - releasedAt) * 1000
            NSLog("Vakyu voice round %llu: audio drained in %.0f ms", epoch, elapsed)
            self.connection.send(["command": "release"])
        }
    }

    func cancel() {
        queuedPress = nil
        completionPresentation.reset()
        discardUntilHide = model.visible || pressed
        roundEpoch &+= 1
        holdWork?.cancel(); holdWork = nil; pressed = false; pressedMode = nil
        microphone.stop(); activeGeneration = nil; speech.cancel()
        connection.send(["command": "cancel"])
        delivery.reset(); model.clearRound(); overlay.hideRound()
    }

    func action(_ action: String, index: Int? = nil) {
        if action == "copy-result" { copyLastResult(); return }
        if action == "close-result" { collapseFailedResult(); return }
        var command: [String: Any] = ["command": "action", "action": action]
        if let index { command["index"] = index }
        if action == "retry" {
            model.error = ""
            if model.commitFailed {
                let epoch = roundEpoch
                completionPresentation.presentFailure { [weak self] in
                    guard let self, self.roundEpoch == epoch else { return }
                    self.collapseFailedResult()
                }
            }
        }
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
        queuedPress = nil
        completionPresentation.reset()
        discardUntilHide = false
        roundEpoch &+= 1; pressed = false; pressedMode = nil; holdWork?.cancel()
        microphone.stop(); activeGeneration = nil; speech.cancel()
        model.ready = false; model.recording = false; model.status = "语音服务未就绪"
        refreshRuntimeStatus()
        showError(message)
        scheduleAutomaticRestart()
        connection.stop()
    }

    func showError(_ message: String) {
        model.error = message
        model.settingMessage = message
        if model.commitFailed {
            model.status = delivery.failureStatus
        }
        if model.visible && model.commitFailed && completionPresentation.collapsed {
            statusItem.showNotice(message)
        } else if model.visible { overlay.show(practice: model.practice != nil) }
        else { statusItem.showNotice(message); overlay.showNotice(message: message) }
    }

    func collapseFailedResult() {
        guard model.visible, model.commitFailed, !model.recording else { return }
        completionPresentation.collapseNow()
        overlay.hideRound()
        statusItem.refresh()
        statusItem.showNotice(model.pastedWithoutVerification
            ? "已粘贴，无法核对；可在菜单中查看或复制识别结果。"
            : "上次填入未完成，可在菜单中查看或复制识别结果。")
    }

    func reopenFailedResult() {
        guard model.visible, model.commitFailed else { return }
        completionPresentation.reset()
        overlay.show(practice: model.practice != nil)
    }

    func copyLastResult() {
        guard !model.lastResult.isEmpty else { return }
        let board = NSPasteboard.general
        board.clearContents()
        if board.setString(model.lastResult, forType: .string) {
            statusItem.showNotice("已复制上次识别文字。")
        }
    }

    func restart() {
        cancel()
        keyboard.stop()
        refreshPermissions()
        model.ready = false; model.status = "正在重新启动语音服务…"
        refreshRuntimeStatus()
        restartRequested = true
        connection.stop()
    }

    func shutdown() {
        shuttingDown = true
        restartRequested = false
        cancel(); keyboard.stop(); reservation.release(); connection.stop()
    }

    /// 5 分钟内最多自动重启 3 次；持续失败多半是组件损坏或机器资源不足，反复重启只会更卡。
    func scheduleAutomaticRestart() {
        guard !shuttingDown else { return }
        let now = Date()
        automaticRestarts = automaticRestarts.filter { now.timeIntervalSince($0) < 300 }
        guard automaticRestarts.count < 3 else {
            NSLog("Vakyu voice service: automatic restart limit reached")
            return
        }
        automaticRestarts.append(now)
        automaticRestartPending = true
        restartRequested = true
        NSLog("Vakyu voice service: automatic restart scheduled attempt=%ld", automaticRestarts.count)
    }
}
