// 只在用户点击授权时请求系统权限，状态返回设置页可见提示。
import AVFoundation
import ApplicationServices
import AppKit

enum PermissionState {
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
    }

    static func requestKeyboard() { _ = CGRequestListenEventAccess(); open("ListenEvent") }

    static func open(_ pane: String) {
        if let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_\(pane)") {
            NSWorkspace.shared.open(url)
        }
    }
}
