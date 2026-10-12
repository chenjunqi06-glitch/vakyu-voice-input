//! 投递的系统边界；内存替身可回放真实状态机而不访问 AX、剪贴板或键盘。
import Foundation

protocol TextDeliveryAccess {
    func validate(_ target: InputTarget, protectDraft: Bool) throws

    func snapshot(_ target: InputTarget) -> SelectionSnapshot

    func editable(_ target: InputTarget, attribute: String) -> Bool

    func write(_ target: InputTarget, attribute: String, text: String)

    func moveCaret(_ target: InputTarget, to position: Int)

    // 抛错只能发生在发送按键之前；返回后可能已写入，必须读回且不得盲目重试。
    func paste(_ target: InputTarget, text: String) throws -> () -> Void
}
