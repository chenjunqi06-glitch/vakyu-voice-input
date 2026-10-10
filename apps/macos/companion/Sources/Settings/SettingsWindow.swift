// 原生语音设置窗口；关闭设置不影响后台语音服务或未完成的跟读。
import AppKit
import SwiftUI

final class SettingsWindow: NSObject, NSWindowDelegate {
    private let window: NSWindow

    private weak var coordinator: VoiceCoordinator?

    init(coordinator: VoiceCoordinator) {
        self.coordinator = coordinator
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 730, height: 680),
                          styleMask: [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView],
                          backing: .buffered, defer: false)
        super.init()
        window.title = "Vakyu 语音设置"
        window.titlebarAppearsTransparent = true
        window.isReleasedWhenClosed = false
        window.minSize = NSSize(width: 610, height: 580)
        window.backgroundColor = NSColor(calibratedRed: 0.25, green: 0.14, blue: 0.46, alpha: 1)
        window.appearance = NSAppearance(named: .darkAqua)
        window.contentView = NSHostingView(rootView: VoiceSettingsView(coordinator: coordinator, model: coordinator.model))
        window.delegate = self
        window.center()
    }

    func show() {
        NSApp.activate(ignoringOtherApps: true)
        window.makeKeyAndOrderFront(nil)
    }

    func windowWillClose(_ notification: Notification) {
        coordinator?.keyboard.endRecording()
        coordinator?.model.shortcutRecording = false
    }
}
