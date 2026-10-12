//! 只读取前台应用的可见普通窗口标识，不读取窗口标题或图像。
import AppKit

enum FrontWindow {
    static func identifier(processID: pid_t) -> CGWindowID? {
        guard NSWorkspace.shared.frontmostApplication?.processIdentifier == processID,
              let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID)
                as? [[String: Any]] else { return nil }
        for window in windows {
            guard (window[kCGWindowOwnerPID as String] as? NSNumber)?.int32Value == processID,
                  (window[kCGWindowLayer as String] as? NSNumber)?.intValue == 0,
                  (window[kCGWindowAlpha as String] as? NSNumber)?.doubleValue ?? 1 > 0,
                  let id = (window[kCGWindowNumber as String] as? NSNumber)?.uint32Value,
                  id != kCGNullWindowID else { continue }
            return id
        }
        return nil
    }
}
