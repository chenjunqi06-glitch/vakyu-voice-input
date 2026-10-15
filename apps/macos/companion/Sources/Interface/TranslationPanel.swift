// 划词翻译的结果小窗：出现在鼠标旁，不抢原软件焦点；点窗外任意处或 × 关闭，「复制译文」写入剪贴板。
import AppKit
import SwiftUI

final class TranslationState: ObservableObject {
    enum Phase { case pending, ready, failed }

    @Published var request: UInt64 = 0

    @Published var phase = Phase.pending

    @Published var source = ""

    @Published var text = ""

    @Published var target = "en"

    @Published var copied = false

    var title: String {
        switch phase {
        case .pending: return "Vakyu 翻译中…"
        case .ready: return target == "en" ? "Vakyu · 译成英文" : "Vakyu · 译成中文"
        case .failed: return "Vakyu · 翻译失败"
        }
    }
}

final class TranslationPanel: NSPanel {
    private static let size = NSSize(width: 380, height: 250)

    private let state = TranslationState()

    private var monitor: Any?

    override var canBecomeKey: Bool { false }

    override var canBecomeMain: Bool { false }

    init() {
        super.init(contentRect: NSRect(origin: .zero, size: Self.size),
                   styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
        isReleasedWhenClosed = false
        isOpaque = false; backgroundColor = .clear; hasShadow = true
        hidesOnDeactivate = false
        level = .floating
        collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .transient]
        animationBehavior = .none
        contentView = TranslationHostingView(rootView: TranslationView(
            state: state, copy: { [weak self] in self?.copyTranslation() }, close: { [weak self] in self?.dismiss() }))
    }

    /// 新请求替换旧窗口内容；旧请求晚到的结果按 id 丢弃。
    func present(request: UInt64, source: String, at point: NSPoint) {
        state.request = request
        state.phase = .pending
        state.source = source
        state.text = "正在翻译…"
        state.copied = false
        let screen = NSScreen.screens.first { NSMouseInRect(point, $0.frame, false) } ?? NSScreen.main
        var origin = NSPoint(x: point.x - 24, y: point.y - Self.size.height - 18)
        if let visible = screen?.visibleFrame {
            origin.x = min(max(origin.x, visible.minX + 8), visible.maxX - Self.size.width - 8)
            if origin.y < visible.minY + 8 { origin.y = min(point.y + 18, visible.maxY - Self.size.height - 8) }
        }
        setFrame(NSRect(origin: origin, size: Self.size), display: true)
        orderFrontRegardless()
        if monitor == nil {
            // 全局监听只收到其他软件里的点击，点本窗口自己的按钮不会触发关闭。
            monitor = NSEvent.addGlobalMonitorForEvents(matching: [.leftMouseDown, .rightMouseDown]) { [weak self] _ in
                DispatchQueue.main.async { self?.dismiss() }
            }
        }
    }

    var pendingRequest: UInt64? { isVisible && state.phase == .pending ? state.request : nil }

    func finish(request: UInt64, ok: Bool, text: String, target: String) {
        guard isVisible, state.request == request, state.phase == .pending else { return }
        state.phase = ok ? .ready : .failed
        state.text = text
        state.target = target
    }

    func dismiss() {
        if let monitor { NSEvent.removeMonitor(monitor) }
        monitor = nil
        orderOut(nil)
    }

    private func copyTranslation() {
        guard state.phase == .ready else { return }
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(state.text, forType: .string)
        state.copied = true
    }
}

final class TranslationHostingView: NSHostingView<TranslationView> {
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
}

struct TranslationView: View {
    @ObservedObject var state: TranslationState

    let copy: () -> Void

    let close: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text(state.title).font(.system(size: 13, weight: .bold, design: .rounded))
                Spacer()
                Button(action: close) {
                    Image(systemName: "xmark").font(.system(size: 11, weight: .bold))
                }
                .buttonStyle(.plain).help("关闭")
            }
            Text(state.source)
                .font(.system(size: 12)).foregroundStyle(Color.white.opacity(0.6))
                .lineLimit(2).truncationMode(.tail)
            ScrollView {
                Text(state.text)
                    .font(.system(size: 15))
                    .foregroundStyle(state.phase == .failed ? Color(red: 1, green: 0.87, blue: 0.65) : Color.white)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .fixedSize(horizontal: false, vertical: true)
                    .textSelection(.enabled)
            }
            HStack {
                Text("只读取选中的文字，不改动原文").font(.caption2).foregroundStyle(Color.white.opacity(0.5))
                Spacer()
                Button(state.copied ? "已复制" : "复制译文", action: copy)
                    .disabled(state.phase != .ready)
            }
        }
        .foregroundStyle(.white)
        .padding(16)
        .frame(width: 380, height: 250)
        .background(Brand.gradient, in: RoundedRectangle(cornerRadius: 16))
        .overlay(RoundedRectangle(cornerRadius: 16).stroke(Color.white.opacity(0.18), lineWidth: 1))
    }
}
