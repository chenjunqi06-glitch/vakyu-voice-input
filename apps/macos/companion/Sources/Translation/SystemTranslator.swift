// 系统翻译：macOS 15 起的 Translation 框架，完全在本机运行，不花钱、不需要 API 密钥。
// 语言没下载时不弹系统下载窗（隐藏的宿主窗口里弹不出来），改为提示到系统设置下载，下载后离线可用。
import AppKit
import SwiftUI
#if canImport(Translation)
import Translation
#endif

final class SystemTranslator {
    struct Failure {
        let message: String

        /// 这台 Mac 没有系统翻译（系统版本太旧或不支持这对语言），而不是语言还没下载。
        let unavailable: Bool
    }

    private var host: AnyObject?

    /// `target` 为 `en` 或 `zh`；回调总在主线程。
    func translate(_ text: String, target: String, completion: @escaping (String?, Failure?) -> Void) {
        #if canImport(Translation)
        if #available(macOS 15.0, *) {
            let host = (self.host as? SystemTranslationHost) ?? SystemTranslationHost()
            self.host = host
            host.enqueue(text: text, target: target, completion: completion)
            return
        }
        #endif
        completion(nil, Failure(message: "翻译需要 macOS 15 或更新版本的系统翻译，请升级系统后再用。", unavailable: true))
    }

    /// 划词定方向：汉字多于拉丁字母译成英文，否则译成中文（与服务端规则一致）。
    static func readingTarget(_ text: String) -> String {
        var han = 0, latin = 0
        for scalar in text.unicodeScalars {
            switch scalar.value {
            case 0x3400...0x4DBF, 0x4E00...0x9FFF, 0xF900...0xFAFF, 0x20000...0x323AF: han += 1
            case 0x41...0x5A, 0x61...0x7A, 0xC0...0x24F, 0x1E00...0x1EFF: latin += 1
            default: break
            }
        }
        return han > latin ? "en" : "zh"
    }
}

#if canImport(Translation)
@available(macOS 15.0, *)
final class SystemTranslationHost: ObservableObject {
    struct Job {
        let text: String

        let source: Locale.Language

        let target: Locale.Language

        let completion: (String?, SystemTranslator.Failure?) -> Void
    }

    @Published var configuration: TranslationSession.Configuration?

    private var pending: [Job] = []

    private var current: (source: Locale.Language, target: Locale.Language)?

    private var triggeredAt = Date.distantPast

    private var window: NSPanel?

    static let downloadHint = "第一次使用需要下载系统翻译语言：打开“系统设置 → 通用 → 语言与地区 → 翻译语言”，下载“中文（简体）”和“英语”，之后离线可用。"

    func enqueue(text: String, target: String, completion: @escaping (String?, SystemTranslator.Failure?) -> Void) {
        ensureWindow()
        let english = Locale.Language(identifier: "en")
        let chinese = Locale.Language(identifier: "zh-Hans")
        let source = target == "en" ? chinese : english
        let destination = target == "en" ? english : chinese
        Task {
            let status = await LanguageAvailability().status(from: source, to: destination)
            await MainActor.run {
                switch status {
                case .installed:
                    self.pending.append(Job(text: text, source: source, target: destination, completion: completion))
                    // 上一轮 30 秒还没跑完（视图任务被系统挂起等）就重新触发，避免请求一直排队。
                    if self.current == nil || Date().timeIntervalSince(self.triggeredAt) > 30 {
                        self.trigger(source: source, target: destination)
                    }
                case .supported:
                    completion(nil, SystemTranslator.Failure(message: Self.downloadHint, unavailable: false))
                default:
                    completion(nil, SystemTranslator.Failure(message: "系统翻译不支持中英互译。", unavailable: true))
                }
            }
        }
    }

    /// 由隐藏视图的 translationTask 调用：处理方向与本次会话一致的请求，剩下的换方向再来一轮。
    func run(_ session: TranslationSession) async {
        let jobs: [Job] = await MainActor.run {
            guard let current = self.current else { return [] }
            let ready = self.pending.filter { $0.source == current.source && $0.target == current.target }
            self.pending.removeAll { $0.source == current.source && $0.target == current.target }
            return ready
        }
        for job in jobs {
            do {
                let response = try await session.translate(job.text)
                let text = response.targetText
                await MainActor.run { job.completion(text, nil) }
            } catch {
                let message = "系统翻译失败：\(error.localizedDescription)"
                await MainActor.run { job.completion(nil, SystemTranslator.Failure(message: message, unavailable: false)) }
            }
        }
        await MainActor.run {
            self.current = nil
            if let next = self.pending.first { self.trigger(source: next.source, target: next.target) }
        }
    }

    private func trigger(source: Locale.Language, target: Locale.Language) {
        current = (source, target)
        triggeredAt = Date()
        if configuration?.source == source && configuration?.target == target {
            configuration?.invalidate()
        } else {
            configuration = TranslationSession.Configuration(source: source, target: target)
        }
    }

    /// translationTask 需要挂在显示中的视图上；用透明、不接收点击的 1×1 窗口承载。
    private func ensureWindow() {
        guard window == nil else { return }
        let panel = NSPanel(contentRect: NSRect(x: 0, y: 0, width: 1, height: 1),
                            styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
        panel.isReleasedWhenClosed = false
        panel.alphaValue = 0
        panel.ignoresMouseEvents = true
        panel.hasShadow = false
        panel.collectionBehavior = [.canJoinAllSpaces, .stationary, .ignoresCycle]
        panel.contentView = NSHostingView(rootView: SystemTranslationView(host: self))
        panel.orderFrontRegardless()
        window = panel
    }
}

@available(macOS 15.0, *)
struct SystemTranslationView: View {
    @ObservedObject var host: SystemTranslationHost

    var body: some View {
        Color.clear
            .frame(width: 1, height: 1)
            .translationTask(host.configuration) { session in
                await host.run(session)
            }
    }
}
#endif
