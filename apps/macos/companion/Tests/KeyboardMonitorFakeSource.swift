//! 本地事件回放来源，不安装系统钩子、不发送真实按键。
import ApplicationServices
@testable import VakyuVoice

final class KeyboardMonitorFakeSource: KeyboardEventSource {
    var available = false

    var mayStart = true

    var starts = 0

    var handler: ((CGEventType, CGEvent) -> Bool)?

    func start(_ handler: @escaping (CGEventType, CGEvent) -> Bool) -> Bool {
        starts += 1
        self.handler = handler
        available = mayStart
        return available
    }

    func stop() { available = false; handler = nil }

    @discardableResult
    func send(_ type: CGEventType, code: UInt16 = 0, flags: CGEventFlags = []) -> Bool {
        let event = CGEvent(source: nil)!
        event.type = .keyDown
        event.setIntegerValueField(.keyboardEventKeycode, value: Int64(code))
        event.flags = flags
        return handler?(type, event) ?? false
    }
}
