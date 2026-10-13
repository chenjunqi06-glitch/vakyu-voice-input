// 划词翻译的请求与回执；与语音轮次互不影响，不写入任何输入框。
import AppKit

extension VoiceCoordinator {
    func translate(_ text: String) {
        translationRequest &+= 1
        let request = translationRequest
        translationPanel.present(request: request, source: text, at: NSEvent.mouseLocation)
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
}
