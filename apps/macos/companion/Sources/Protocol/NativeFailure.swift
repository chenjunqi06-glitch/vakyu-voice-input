// 本机错误仅包含可读提示，避免把录音和配置原文泄漏到系统日志。
import Foundation

struct NativeFailure: LocalizedError {
    let message: String

    init(_ message: String) { self.message = message }

    var errorDescription: String? { message }
}
