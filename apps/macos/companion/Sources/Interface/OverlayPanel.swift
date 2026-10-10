// nonactivatingPanel 保留原应用焦点，支持全屏 Space 与多显示器可见区域。
import AppKit
import SwiftUI

final class OverlayPanel: NSPanel {
    override var canBecomeKey: Bool { false }

    override var canBecomeMain: Bool { false }

    private var presentationScreen: NSScreen?

    init(model: VoiceModel, action: @escaping (String, Int?) -> Void) {
        super.init(contentRect: .zero, styleMask: [.borderless, .nonactivatingPanel],
                   backing: .buffered, defer: false)
        isReleasedWhenClosed = false
        isOpaque = false; backgroundColor = .clear; hasShadow = false
        hidesOnDeactivate = false
        level = .statusBar
        collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary]
        isMovable = false
        animationBehavior = .none
        contentView = OverlayHostingView(rootView: VoicePanelView(model: model, action: action))
    }

    func show(practice: Bool, lockScreen: Bool = false) {
        if presentationScreen == nil || lockScreen {
            let point = NSEvent.mouseLocation
            presentationScreen = NSScreen.screens.first { NSMouseInRect(point, $0.frame, false) } ?? NSScreen.main
        }
        if let saved = presentationScreen, !NSScreen.screens.contains(where: { $0 == saved }) {
            presentationScreen = NSScreen.main
        }
        guard let screen = presentationScreen ?? NSScreen.main else { return }
        let available = screen.visibleFrame
        let width = min(720, max(380, available.width - 30))
        let height = min(practice ? 510.0 : 285.0, available.height - 36)
        let frame = NSRect(x: available.midX - width / 2, y: available.minY + 18, width: width, height: height)
        if self.frame != frame { setFrame(frame, display: true) }
        if !isVisible { orderFrontRegardless() }
    }

    func hideRound() { orderOut(nil); presentationScreen = nil }
}
