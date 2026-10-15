// 按协议处理服务事件；commit 成功以前不会清空窗口或发出任何发送键。
import Foundation

extension VoiceCoordinator {
    func receive(_ object: [String: Any]) {
        guard let event = object["event"] as? String else { return }
        guard serviceHealthy else { return }
        if discardUntilHide && ["capture", "dictation", "practice", "speak", "commit", "error"].contains(event) { return }
        if completionPresentation.collapsed && ["capture", "dictation", "practice", "speak"].contains(event) { return }
        switch event {
        case "ready":
            model.ready = true; model.status = "语音模型就绪"; refreshRuntimeStatus()
        case "config":
            applyConfiguration(VoiceConfiguration(object))
        case "capture":
            guard model.visible else { return }
            guard let generation = (object["generation"] as? NSNumber)?.uint64Value else { return }
            if object["active"] as? Bool == true {
                if model.practice == nil && !pressed {
                    connection.send(["command": "release"])
                    return
                }
                do {
                    guard let target = delivery.target else { throw NativeFailure("没有可用的原输入框，请重新录音。") }
                    try target.validate(requireFocus: false)
                    activeGeneration = generation
                    model.recording = true
                    microphone.start(generation: generation)
                } catch {
                    activeGeneration = generation
                    captureFailed(error.localizedDescription)
                }
            } else if activeGeneration == generation {
                microphone.stop(); activeGeneration = nil; model.recording = false
                model.meter.reset()
            }
        case "dictation":
            let text = object["text"] as? String ?? ""
            // 服务队列可能晚到一帧聆听状态；松键后不再恢复本轮采音提示。
            let recording = (object["recording"] as? Bool ?? false) && pressed
            let status = recording ? "正在聆听" : "正在处理…"
            if model.dictation != text { model.dictation = text }
            if model.recording != recording { model.recording = recording }
            if model.status != status { model.status = status }
            if !model.visible { model.visible = true }
            overlay.show(practice: false)
        case "practice":
            guard let raw = object["view"], let view = PracticeSnapshot.decode(raw) else {
                showError("跟读状态格式异常，请重新启动语音服务。")
                return
            }
            if model.practice != view { model.practice = view }
            if model.recording != view.recording { model.recording = view.recording }
            if !model.commitFailed && model.status != view.status { model.status = view.status }
            if !model.visible { model.visible = true }
            overlay.show(practice: true)
        case "speak":
            guard let id = (object["id"] as? NSNumber)?.uint64Value,
                  let text = object["text"] as? String else { return }
            microphone.stop(); activeGeneration = nil; model.recording = false
            speech.play(id: id, text: text, language: object["language"] as? String ?? "en",
                        rate: object["rate"] as? Int ?? -1)
        case "cancel_speech": speech.cancel()
        case "commit":
            guard let id = (object["id"] as? NSNumber)?.uint64Value,
                  let text = object["text"] as? String else { return }
            let epoch = roundEpoch
            completionPresentation.reset()
            model.commitFailed = false
            model.lastResult = text
            statusItem.refresh()
            delivery.commit(id: id, text: text) { [weak self] ok, message in
                guard let self, self.roundEpoch == epoch else { return }
                self.model.commitFailed = !ok
                self.model.pastedWithoutVerification = self.delivery.pastedWithoutVerification
                NSLog("Vakyu voice round %llu: commit ok=%@", epoch, String(ok))
                if !ok {
                    self.model.recording = false
                    self.model.meter.reset()
                    self.model.status = self.delivery.failureStatus
                    self.showError(message)
                    self.completionPresentation.presentFailure { [weak self] in
                        guard let self, self.roundEpoch == epoch else { return }
                        self.collapseFailedResult()
                    }
                    self.statusItem.refresh()
                }
                self.connection.send(["command": "commit_result", "id": id, "ok": ok, "message": message])
            }
        case "hide":
            let next = queuedPress
            queuedPress = nil
            completionPresentation.reset()
            discardUntilHide = false
            roundEpoch &+= 1; pressed = false; pressedMode = nil; holdWork?.cancel(); holdWork = nil
            microphone.stop(); activeGeneration = nil; speech.cancel()
            delivery.reset(); model.clearRound(); overlay.hideRound()
            statusItem.refresh()
            if let next, keyboard.heldMode == next { keyPressed(mode: next) }
        case "error": showError(object["message"] as? String ?? "语音服务暂时不可用。")
        case "setting_result": settingResult(object)
        case "translate_request": translateForService(object)
        default: break
        }
    }

    func applyConfiguration(_ config: VoiceConfiguration) {
        model.configuration = config
        delivery.pasteFallbackEnabled = config.pasteFallback
        if (!config.enabled || (!config.practice && model.entryMode == .practice)) && model.visible {
            holdWork?.cancel(); holdWork = nil; pressed = false; pressedMode = nil
            microphone.stop(); activeGeneration = nil; model.recording = false
        }
        synchronizeShortcut()
        refreshRuntimeStatus()
        statusItem.refresh()
    }

    func synchronizeShortcut() {
        let config = model.configuration
        let practiceKey = (try? Shortcut(config.shortcut)) ?? keyboard.shortcut
        let collision = VoiceKeyRouter.conflicts(dictationKey: config.dictationKey, practice: practiceKey)
        keyboard.configure(enabled: config.enabled,
                           practiceEnabled: config.practice && !collision,
                           dictationKey: config.dictationKey, shortcut: keyboard.shortcut)
        if collision { model.shortcutMessage = "跟读快捷键与普通语音键重叠，请为跟读选择其他快捷键。普通语音仍可用。" }
        guard config.enabled, config.practice, !collision, keyboard.available else {
            reservation.release(); lastConfiguredShortcut = nil
            keyboard.configure(enabled: config.enabled, practiceEnabled: false,
                               dictationKey: config.dictationKey, shortcut: practiceKey)
            return
        }
        if lastConfiguredShortcut != config.shortcut && pendingShortcut == nil {
            do {
                let candidate = try Shortcut(config.shortcut)
                try reservation.replace(with: candidate)
                keyboard.configure(enabled: config.enabled, practiceEnabled: true,
                                   dictationKey: config.dictationKey, shortcut: candidate)
                lastConfiguredShortcut = config.shortcut
                model.shortcutMessage = candidate.modifierOnly
                    ? "纯修饰键无法检测系统占用；其他软件内部的快捷键也可能无法发现。" : ""
            } catch {
                model.shortcutMessage = error.localizedDescription
                keyboard.configure(enabled: config.enabled, practiceEnabled: false,
                                   dictationKey: config.dictationKey, shortcut: keyboard.shortcut)
            }
        }
    }
}
