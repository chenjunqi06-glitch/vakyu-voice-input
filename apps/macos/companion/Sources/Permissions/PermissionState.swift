// 只在用户点击授权时请求系统权限，状态返回设置页可见提示。
import AVFoundation
import ApplicationServices
import AppKit

enum PermissionState {
    private(set) static var accessibilityMayBeStale = false

    static let staleAccessibilityMessage = "辅助功能权限可能已失效，请在系统设置里移除后重新添加 Vakyu 语音。"

    static func observeFocusError(_ error: AXError) {
        if error == .apiDisabled || error == .cannotComplete {
            accessibilityMayBeStale = accessibility
        } else if error == .success {
            accessibilityMayBeStale = false
        }
    }

    static var microphone: Bool { AVCaptureDevice.authorizationStatus(for: .audio) == .authorized }

    static var accessibility: Bool { AXIsProcessTrusted() }

    static var keyboard: Bool { CGPreflightListenEventAccess() }

    static func requestMicrophone(_ done: @escaping () -> Void) {
        if AVCaptureDevice.authorizationStatus(for: .audio) == .notDetermined {
            AVCaptureDevice.requestAccess(for: .audio) { _ in DispatchQueue.main.async(execute: done) }
        } else { open("Microphone"); done() }
    }

    static func requestAccessibility() {
        let options = [kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String: true] as CFDictionary
        _ = AXIsProcessTrustedWithOptions(options)
        open("Accessibility")
    }

    static func requestKeyboard() { _ = CGRequestListenEventAccess(); open("ListenEvent") }

    static var applicationURL: URL { Bundle.main.bundleURL.resolvingSymlinksInPath() }

    static func revealApplication() {
        NSWorkspace.shared.activateFileViewerSelecting([applicationURL])
    }

    static func copyApplicationPath() {
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        pasteboard.setString(applicationURL.path, forType: .string)
    }

    static func open(_ pane: String) {
        if let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_\(pane)") {
            NSWorkspace.shared.open(url)
        }
    }
}
