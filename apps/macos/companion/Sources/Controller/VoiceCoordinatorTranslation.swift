// 翻译的请求与回执：默认用系统翻译（本机、免费），设置为云端或本机没有系统翻译时交给服务调用接口。
// 与语音轮次互不影响，划词结果不写入任何输入框。
import AppKit

extension VoiceCoordinator {
    /// 划词翻译（右键「用 Vakyu 翻译」）。
    func translate(_ text: String) {
        translationRequest &+= 1
        let request = translationRequest
        translationPanel.present(request: request, source: text, at: NSEvent.mouseLocation)
        guard model.configuration.translationEngine != "cloud" else {
            translateWithService(request: request, text: text)
            return
        }
        let target = SystemTranslator.readingTarget(text)
        systemTranslator.translate(text, target: target) { [weak self] result, failure in
            guard let self else { return }
            if let result {
                self.translationPanel.finish(request: request, ok: true, text: result, target: target)
            } else if failure?.unavailable == true {
                // 系统没有翻译功能：填了密钥就走云端，没填由服务回报原因。
                self.translateWithService(request: request, text: text)
            } else {
                self.translationPanel.finish(request: request, ok: false,
                                             text: failure?.message ?? "系统翻译失败，请重试。", target: target)
            }
        }
    }

    private func translateWithService(request: UInt64, text: String) {
        guard serviceHealthy else {
            translationPanel.finish(request: request, ok: false,
                                    text: "语音服务未就绪，请在菜单栏 Vakyu 图标里重新启动语音服务。", target: "en")
            return
        }
        connection.send(["command": "translate", "id": request, "text": text])
        // 服务端自身有两分钟网络上限；再留余量，避免进程异常时窗口一直停在「翻译中」。
        DispatchQueue.main.asyncAfter(deadline: .now() + 130) { [weak self] in
            guard let self, self.translationPanel.pendingRequest == request else { return }
            self.translationPanel.finish(request: request, ok: false, text: "翻译等待超时，请检查网络后重试。", target: "en")
        }
    }

    func translationResult(_ object: [String: Any]) {
        guard let request = (object["id"] as? NSNumber)?.uint64Value else { return }
        translationPanel.finish(request: request,
                                ok: object["ok"] as? Bool ?? false,
                                text: object["text"] as? String ?? "翻译失败，请重试。",
                                target: object["target"] as? String ?? "en")
    }

    /// 跟读：服务请宿主用系统翻译本轮原话，结果以 `translated` 命令回报；服务按轮次丢弃过期结果。
    func translateForService(_ object: [String: Any]) {
        guard let id = (object["id"] as? NSNumber)?.uint64Value,
              let text = object["text"] as? String else { return }
        let target = object["target"] as? String ?? "en"
        systemTranslator.translate(text, target: target) { [weak self] result, failure in
            var reply: [String: Any] = ["command": "translated", "id": id]
            if let result {
                reply["ok"] = true
                reply["text"] = result
            } else {
                reply["ok"] = false
                reply["text"] = failure?.message ?? "系统翻译失败，请重试。"
                reply["fallback"] = failure?.unavailable ?? false
            }
            self?.connection.send(reply)
        }
    }
}
