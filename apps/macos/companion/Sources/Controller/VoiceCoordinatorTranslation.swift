// 翻译只用 macOS 系统翻译（本机运行，不联网、不花钱）；与语音轮次互不影响，划词结果不写入任何输入框。
import AppKit

extension VoiceCoordinator {
    /// 划词翻译（右键「用 Vakyu 翻译」）。
    func translate(_ text: String) {
        translationRequest &+= 1
        let request = translationRequest
        translationPanel.present(request: request, source: text, at: NSEvent.mouseLocation)
        let target = SystemTranslator.readingTarget(text)
        systemTranslator.translate(text, target: target) { [weak self] result, failure in
            guard let self else { return }
            self.translationPanel.finish(request: request, ok: result != nil,
                                         text: result ?? failure?.message ?? "系统翻译失败，请重试。",
                                         target: target)
        }
        // 系统翻译的后台任务偶尔会被挂起；一分钟没结果就提示重试，避免窗口一直停在「翻译中」。
        DispatchQueue.main.asyncAfter(deadline: .now() + 60) { [weak self] in
            guard let self, self.translationPanel.pendingRequest == request else { return }
            self.translationPanel.finish(request: request, ok: false, text: "系统翻译没有响应，请重试。", target: target)
        }
    }

    /// 跟读：服务请宿主用系统翻译本轮原话，结果以 `translated` 命令回报；服务按轮次丢弃过期结果。
    func translateForService(_ object: [String: Any]) {
        guard let id = (object["id"] as? NSNumber)?.uint64Value,
              let text = object["text"] as? String else { return }
        let target = object["target"] as? String ?? "en"
        systemTranslator.translate(text, target: target) { [weak self] result, failure in
            self?.connection.send([
                "command": "translated",
                "id": id,
                "ok": result != nil,
                "text": result ?? failure?.message ?? "系统翻译失败，请重试。"
            ])
        }
    }
}
