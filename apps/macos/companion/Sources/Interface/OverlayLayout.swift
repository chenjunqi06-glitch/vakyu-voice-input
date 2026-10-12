// 浮窗占用原尺寸的一半；正文保持可读字号，长内容在卡片内滚动。
import AppKit

enum OverlayLayout {
    static let width: CGFloat = 360

    static let dictationHeight: CGFloat = 142.5

    static let practiceHeight: CGFloat = 255

    static func noticeHeight(message: String) -> CGFloat {
        // 提示不接收鼠标事件，须按正文展开，不能依赖滚动查看错误。
        let textBounds = (message as NSString).boundingRect(
            with: NSSize(width: width - 48, height: .greatestFiniteMagnitude),
            options: [.usesLineFragmentOrigin, .usesFontLeading],
            attributes: [.font: NSFont.systemFont(ofSize: 12)])
        return max(110, ceil(textBounds.height) + 76)
    }
}
