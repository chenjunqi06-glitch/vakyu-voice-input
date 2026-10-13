// 系统「服务」菜单（右键菜单）里的「用 Vakyu 翻译」：任何软件选中文字都能用，只读取选中的文字，不改动原文。
import AppKit
import CoreServices

final class TranslationService: NSObject {
    var onText: ((String) -> Void)?

    func register() {
        NSApp.servicesProvider = self
        // 语音宿主嵌在输入法包里，先确保系统登记了这个 app，再让服务菜单重新扫描 Info.plist 的 NSServices。
        _ = LSRegisterURL(Bundle.main.bundleURL as CFURL, true)
        NSUpdateDynamicServices()
    }

    /// Info.plist 里 NSMessage = translateSelection；选择器必须是 translateSelection:userData:error:。
    @objc func translateSelection(_ pasteboard: NSPasteboard, userData: String?,
                                  error: AutoreleasingUnsafeMutablePointer<NSString>) {
        guard let text = pasteboard.string(forType: .string)?
            .trimmingCharacters(in: .whitespacesAndNewlines), !text.isEmpty else {
            error.pointee = "没有读到选中的文字。" as NSString
            return
        }
        onText?(text)
    }
}
