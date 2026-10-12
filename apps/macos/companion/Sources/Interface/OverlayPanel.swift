// nonactivatingPanel 保留原应用焦点，支持全屏 Space 与多显示器可见区域。
import AppKit
import SwiftUI

final class OverlayPanel: NSPanel {
    override var canBecomeKey: Bool { false }

    override var canBecomeMain: Bool { false }

    private var presentationScreen: NSScreen?

    private let hostingView: OverlayHostingView

    private var noticeTimer: Timer?

    private var noticeEpoch: UInt64 = 0

    init(model: VoiceModel, action: @escaping (String, Int?) -> Void) {
        hostingView = OverlayHostingView(rootView: VoicePanelView(model: model, action: action))
        hostingView.sizingOptions = []
        super.init(contentRect: .zero, styleMask: [.borderless, .nonactivatingPanel],
                   backing: .buffered, defer: false)
        isReleasedWhenClosed = false
        isOpaque = false; backgroundColor = .clear; hasShadow = false
        hidesOnDeactivate = false
        level = .statusBar
        collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary]
        isMovable = false
        animationBehavior = .none
        contentView = hostingView
    }

    func show(practice: Bool, lockScreen: Bool = false) {
        cancelNotice()
        present(height: practice ? OverlayLayout.practiceHeight : OverlayLayout.dictationHeight,
                lockScreen: lockScreen)
    }

    /// 不改变会话状态或输入焦点；下一次正式展示会撤销提示的自动隐藏。
    func showNotice(message: String) {
        guard !hostingView.rootView.model.visible else { return }
        cancelNotice()
        var view = hostingView.rootView
        view.notice = message
        hostingView.rootView = view
        ignoresMouseEvents = true
        present(height: OverlayLayout.noticeHeight(message: message), lockScreen: true)
        let epoch = noticeEpoch
        let timer = Timer(timeInterval: 5, repeats: false) { [weak self] _ in
            guard let self, self.noticeEpoch == epoch else { return }
            self.hideRound()
        }
        noticeTimer = timer
        RunLoop.main.add(timer, forMode: .common)
    }

    private func cancelNotice() {
        noticeEpoch &+= 1
        noticeTimer?.invalidate(); noticeTimer = nil
        if hostingView.rootView.notice != nil {
            var view = hostingView.rootView
            view.notice = nil
            hostingView.rootView = view
        }
        ignoresMouseEvents = false
    }

    private func present(height desiredHeight: CGFloat, lockScreen: Bool) {
        if presentationScreen == nil || lockScreen {
            let point = NSEvent.mouseLocation
            presentationScreen = NSScreen.screens.first { NSMouseInRect(point, $0.frame, false) } ?? NSScreen.main
        }
        if let saved = presentationScreen, !NSScreen.screens.contains(where: { $0 == saved }) {
            presentationScreen = NSScreen.main
        }
        guard let screen = presentationScreen ?? NSScreen.main else { return }
        let available = screen.visibleFrame
        let width = min(OverlayLayout.width, max(1, available.width - 30))
        let height = min(desiredHeight, available.height - 36)
        let frame = NSRect(x: available.midX - width / 2, y: available.minY + 18, width: width, height: height)
        if self.frame != frame { setFrame(frame, display: true) }
        if !isVisible { orderFrontRegardless() }
    }

    func hideRound() { cancelNotice(); orderOut(nil); presentationScreen = nil }

    deinit { noticeTimer?.invalidate() }
}
