// 菜单栏后台宿主生命周期与重复启动处理。
import AppKit

final class AppDelegate: NSObject, NSApplicationDelegate {
    private var coordinator: VoiceCoordinator?

    private var observers: [NSObjectProtocol] = []

    private var lease: InstanceLease?

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
        if CommandLine.arguments.contains("--settings") { voice.showSettings() }
    }

    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        coordinator?.showSettings()
        return true
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { false }

    func applicationWillTerminate(_ notification: Notification) { coordinator?.shutdown() }
}
