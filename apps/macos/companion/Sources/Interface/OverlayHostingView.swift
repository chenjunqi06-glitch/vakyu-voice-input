// 非激活窗口第一次点击就可查词，避免第一次被激活行为吃掉。
import AppKit
import SwiftUI

final class OverlayHostingView: NSHostingView<VoicePanelView> {
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    override var acceptsFirstResponder: Bool { false }
}
