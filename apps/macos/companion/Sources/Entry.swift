// 菜单栏语音程序独立运行，不依赖当前输入源或应用内的输入法组件。
import AppKit

@main
enum Entry {
    static func main() {
        // 语音服务退出后再往它的管道写音频会触发 SIGPIPE，默认处理会直接结束整个语音宿主；
        // 忽略后写入改为抛错，由 ServiceConnection 提示并重启服务。
        signal(SIGPIPE, SIG_IGN)
        let app = NSApplication.shared
        let delegate = AppDelegate()
        app.delegate = delegate
        app.setActivationPolicy(.accessory)
        withExtendedLifetime(delegate) { app.run() }
    }
}
