//! 事件钩子的系统资源与健康状态；失效端口必须销毁后重新创建。
import ApplicationServices

final class SystemKeyboardEventSource: KeyboardEventSource {
    private var tap: CFMachPort?

    private var source: CFRunLoopSource?

    private var handler: ((CGEventType, CGEvent) -> Bool)?

    var available: Bool {
        guard let tap else { return false }
        return CFMachPortIsValid(tap) && CGEvent.tapIsEnabled(tap: tap)
    }

    func start(_ handler: @escaping (CGEventType, CGEvent) -> Bool) -> Bool {
        if available { return true }
        stop()
        self.handler = handler
        let mask = [CGEventType.keyDown, .keyUp, .flagsChanged].reduce(CGEventMask(0)) {
            $0 | CGEventMask(1 << $1.rawValue)
        }
        let callback: CGEventTapCallBack = { _, type, event, context in
            guard let context else { return Unmanaged.passUnretained(event) }
            let source = Unmanaged<SystemKeyboardEventSource>.fromOpaque(context).takeUnretainedValue()
            return source.handler?(type, event) == true ? nil : Unmanaged.passUnretained(event)
        }
        guard let newTap = CGEvent.tapCreate(tap: .cgSessionEventTap, place: .headInsertEventTap,
                                            options: .defaultTap, eventsOfInterest: mask,
                                            callback: callback,
                                            userInfo: Unmanaged.passUnretained(self).toOpaque()) else {
            self.handler = nil
            return false
        }
        tap = newTap
        guard let newSource = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, newTap, 0) else {
            stop()
            return false
        }
        source = newSource
        CFRunLoopAddSource(CFRunLoopGetMain(), newSource, .commonModes)
        CGEvent.tapEnable(tap: newTap, enable: true)
        guard available else { stop(); return false }
        return true
    }

    func stop() {
        if let tap { CGEvent.tapEnable(tap: tap, enable: false); CFMachPortInvalidate(tap) }
        if let source { CFRunLoopRemoveSource(CFRunLoopGetMain(), source, .commonModes) }
        tap = nil; source = nil; handler = nil
    }

    deinit { stop() }
}
