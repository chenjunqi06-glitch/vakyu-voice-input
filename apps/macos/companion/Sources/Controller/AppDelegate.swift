// 菜单栏后台宿主生命周期与重复启动处理。
import AppKit

final class AppDelegate: NSObject, NSApplicationDelegate {
    private var coordinator: VoiceCoordinator?

    private var observers: [NSObjectProtocol] = []

    private var lease: InstanceLease?

    private var permissionTimer: Timer?

    private let inputSourceGuard = InputSourceGuard()

    func applicationDidFinishLaunching(_ notification: Notification) {
        do { lease = try InstanceLease() }
        catch {
            if CommandLine.arguments.contains("--settings") {
                DistributedNotificationCenter.default().postNotificationName(
                    NSNotification.Name("app.vakyu.voice.open-settings"), object: nil,
                    userInfo: nil, deliverImmediately: true)
            }
            NSApp.terminate(nil)
            return
        }
        let voice = VoiceCoordinator()
        coordinator = voice
        observers.append(DistributedNotificationCenter.default().addObserver(
            forName: NSNotification.Name("app.vakyu.voice.open-settings"), object: nil, queue: .main
        ) { [weak voice] _ in voice?.showSettings() })
        observers.append(NotificationCenter.default.addObserver(
            forName: NSApplication.didBecomeActiveNotification, object: nil, queue: .main
        ) { [weak voice] _ in voice?.refreshPermissions() })
        observers.append(NotificationCenter.default.addObserver(
            forName: NSApplication.didChangeScreenParametersNotification, object: nil, queue: .main
        ) { [weak voice] _ in
            guard let voice, voice.model.visible else { return }
            voice.overlay.show(practice: voice.model.practice != nil)
        })
        voice.start()
        inputSourceGuard.start()
        // 用户可能在系统设置授权后直接回到聊天软件，不会再次激活语音设置。
        let timer = Timer(timeInterval: 5, repeats: true) { [weak voice] _ in voice?.refreshPermissions() }
        timer.tolerance = 1
        RunLoop.main.add(timer, forMode: .common)
        permissionTimer = timer
        if CommandLine.arguments.contains("--settings") { voice.showSettings() }
    }

    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        coordinator?.showSettings()
        return true
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { false }

    func applicationWillTerminate(_ notification: Notification) {
        permissionTimer?.invalidate()
        inputSourceGuard.stop()
        coordinator?.shutdown()
    }
}
