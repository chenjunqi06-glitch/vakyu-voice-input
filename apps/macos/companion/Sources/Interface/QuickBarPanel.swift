// 常驻桌面的快捷浮窗：一键开关「跟读」与「发英」，不用进设置；左侧把手可拖动，位置与显示状态记在本机偏好里。
import AppKit
import SwiftUI

final class QuickBarPanel: NSPanel {
    static let visibleKey = "quickBar.visible"

    private static let originKey = "quickBar.origin"

    override var canBecomeKey: Bool { false }

    override var canBecomeMain: Bool { false }

    private var observers: [NSObjectProtocol] = []

    init(model: VoiceModel, toggle: @escaping (Bool) -> Void) {
        super.init(contentRect: NSRect(x: 0, y: 0, width: 168, height: 38),
                   styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
        isReleasedWhenClosed = false
        isOpaque = false; backgroundColor = .clear; hasShadow = true
        hidesOnDeactivate = false
        level = .floating
        collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        animationBehavior = .none
        let host = QuickBarHostingView(rootView: QuickBarView(model: model, toggle: toggle))
        contentView = host
        setContentSize(host.fittingSize)
        observers.append(NotificationCenter.default.addObserver(
            forName: NSWindow.didMoveNotification, object: self, queue: .main
        ) { [weak self] _ in self?.saveOrigin() })
        observers.append(NotificationCenter.default.addObserver(
            forName: NSApplication.didChangeScreenParametersNotification, object: nil, queue: .main
        ) { [weak self] _ in self?.place() })
    }

    var isShown: Bool {
        UserDefaults.standard.object(forKey: Self.visibleKey) as? Bool ?? true
    }

    func setShown(_ shown: Bool) {
        UserDefaults.standard.set(shown, forKey: Self.visibleKey)
        restore()
    }

    /// 启动和改开关时调用：按记住的位置摆好；关掉时收起。
    func restore() {
        guard isShown else { orderOut(nil); return }
        place()
        if !isVisible { orderFrontRegardless() }
    }

    private func place() {
        guard let screen = NSScreen.main ?? NSScreen.screens.first else { return }
        let size = frame.size
        var origin = defaultOrigin(on: screen, size: size)
        if let saved = UserDefaults.standard.string(forKey: Self.originKey) {
            let point = NSPointFromString(saved)
            let rect = NSRect(origin: point, size: size)
            // 外接显示器拔掉后旧位置可能在屏幕外，此时回到主屏右上角。
            if NSScreen.screens.contains(where: { $0.visibleFrame.intersects(rect) }) { origin = point }
        }
        if frame.origin != origin { setFrameOrigin(origin) }
    }

    private func defaultOrigin(on screen: NSScreen, size: NSSize) -> NSPoint {
        let visible = screen.visibleFrame
        return NSPoint(x: visible.maxX - size.width - 24, y: visible.maxY - size.height - 12)
    }

    private func saveOrigin() {
        UserDefaults.standard.set(NSStringFromPoint(frame.origin), forKey: Self.originKey)
    }
}

final class QuickBarHostingView: NSHostingView<QuickBarView> {
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    override var acceptsFirstResponder: Bool { false }
}

struct QuickBarView: View {
    @ObservedObject var model: VoiceModel

    let toggle: (Bool) -> Void

    var body: some View {
        let config = model.configuration
        let practiceOn = config.enabled && config.practice
        HStack(spacing: 6) {
            DragHandle()
                .frame(width: 20, height: 24)
                .overlay {
                    Text("V").font(.system(size: 13, weight: .heavy, design: .rounded)).allowsHitTesting(false)
                }
                .help("拖动移动浮窗")
            pill("跟读", on: practiceOn && !config.sendTranslation) { toggle(false) }
                .help(config.learnEnglish ? "说中文 → 跟读英文 → 填入原中文；再点关闭"
                                          : "说英文 → 跟读中文 → 填入原英文；再点关闭")
            pill(config.learnEnglish ? "发英" : "发中", on: practiceOn && config.sendTranslation) { toggle(true) }
                .help(config.learnEnglish ? "说中文 → 跟读英文 → 填入英文；再点关闭"
                                          : "说英文 → 跟读中文 → 填入中文；再点关闭")
        }
        .padding(.horizontal, 8).padding(.vertical, 6)
        .foregroundStyle(.white)
        .background(Brand.gradient, in: Capsule())
        .overlay(Capsule().stroke(Color.white.opacity(0.18), lineWidth: 1))
        .fixedSize()
    }

    private func pill(_ title: String, on: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Text(title)
                .font(.system(size: 12, weight: .semibold))
                .padding(.horizontal, 10).padding(.vertical, 4)
                .foregroundStyle(on ? Color(red: 0.32, green: 0.16, blue: 0.62) : Color.white.opacity(0.85))
                .background(on ? Color.white.opacity(0.95) : Color.white.opacity(0.12), in: Capsule())
        }
        .buttonStyle(.plain)
    }
}

/// 浮窗不能成为 key window，SwiftUI 手势拖不动窗口；交给系统的窗口拖动。
private struct DragHandle: NSViewRepresentable {
    func makeNSView(context: Context) -> NSView { HandleView() }

    func updateNSView(_ nsView: NSView, context: Context) {}

    final class HandleView: NSView {
        override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

        override func mouseDown(with event: NSEvent) { window?.performDrag(with: event) }
    }
}
