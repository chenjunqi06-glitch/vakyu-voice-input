// 菜单栏语音程序独立运行，不依赖当前输入源或应用内的输入法组件。
import AppKit

@main
enum Entry {
    static func main() {
        let app = NSApplication.shared
        let delegate = AppDelegate()
        app.delegate = delegate
        app.setActivationPolicy(.accessory)
        withExtendedLifetime(delegate) { app.run() }
    }
}
