//! 可替换的键盘事件来源，测试可模拟系统停用钩子而不监听真实键盘。
import ApplicationServices

protocol KeyboardEventSource: AnyObject {
    var available: Bool { get }

    func start(_ handler: @escaping (CGEventType, CGEvent) -> Bool) -> Bool

    func stop()
}
