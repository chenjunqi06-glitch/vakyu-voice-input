// 输入法登记自愈：原地升级、回滚或系统重扫后，Vakyu 的输入源 ID 可能在新进程里查不到。
// 查不到时，微信等应用选中 Vakyu 会立刻跳回别的输入源，按 Fn 弹出输入法浮窗时系统还会崩溃（空输入源被释放）。
// 这里只在“查不到”时向系统重新登记已安装的外层输入法包；原本启用过才重新启用，从不切换当前输入源。
import AppKit
import Carbon

final class InputSourceGuard {
    static let parentID = "app.qingjian.inputmethod"

    static let modeID = "app.qingjian.inputmethod.Hans"

    /// 常规检查间隔；查询 TIS 很轻，不读写任何输入内容。
    private static let interval: TimeInterval = 30

    /// 两次修复之间至少间隔，避免系统一直拒绝时反复登记。
    private static let repairCooldown: TimeInterval = 600

    private var timer: Timer?

    private var observers: [NSObjectProtocol] = []

    private var lastRepair: Date?

    func start() {
        // 登录早期系统还在枚举输入源，稍等再查，避免把正常的启动过程当成登记丢失。
        DispatchQueue.main.asyncAfter(deadline: .now() + 5) { [weak self] in self?.check(reason: "launch") }
        let timer = Timer(timeInterval: Self.interval, repeats: true) { [weak self] _ in
            self?.check(reason: "periodic")
        }
        timer.tolerance = 5
        RunLoop.main.add(timer, forMode: .common)
        self.timer = timer
        let center = NSWorkspace.shared.notificationCenter
        for name in [NSWorkspace.didWakeNotification, NSWorkspace.sessionDidBecomeActiveNotification] {
            observers.append(center.addObserver(forName: name, object: nil, queue: .main) { [weak self] _ in
                DispatchQueue.main.asyncAfter(deadline: .now() + 3) { self?.check(reason: "wake") }
            })
        }
    }

    func stop() {
        timer?.invalidate()
        timer = nil
        observers.forEach { NSWorkspace.shared.notificationCenter.removeObserver($0) }
        observers.removeAll()
    }

    private func check(reason: String) {
        let missing = [Self.parentID, Self.modeID].filter { Self.sources(id: $0).isEmpty }
        guard !missing.isEmpty else { return }
        if let lastRepair, Date().timeIntervalSince(lastRepair) < Self.repairCooldown { return }
        guard let bundle = Self.installedInputMethod() else {
            NSLog("Vakyu input source missing=%@ reason=%@; no installed bundle to register",
                  missing.joined(separator: ","), reason)
            return
        }
        lastRepair = Date()
        let wasEnabled = Self.previouslyEnabled()
        let status = TISRegisterInputSource(bundle as CFURL)
        var enabled = 0
        if status == 0 && wasEnabled {
            for source in Self.sources(id: Self.modeID) where !Self.isEnabled(source) {
                if TISEnableInputSource(source) == 0 { enabled += 1 }
            }
        }
        NSLog("Vakyu input source repaired missing=%@ reason=%@ register=%ld previously_enabled=%@ enabled=%ld",
              missing.joined(separator: ","), reason, Int(status), wasEnabled ? "true" : "false", enabled)
    }

    /// 语音宿主嵌在 `Vakyu.app/Contents/Resources/VakyuVoice.app`；只认正式安装目录里的外层包，不登记构建副本。
    static func installedInputMethod(helper: URL = Bundle.main.bundleURL) -> URL? {
        let outer = helper.deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .resolvingSymlinksInPath()
        let allowed = [
            FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Library/Input Methods/Vakyu.app"),
            URL(fileURLWithPath: "/Library/Input Methods/Vakyu.app"),
        ].map { $0.resolvingSymlinksInPath().path }
        guard allowed.contains(outer.path), Bundle(url: outer)?.bundleIdentifier == parentID else { return nil }
        return outer
    }

    /// 用户的启用列表里有 Vakyu，说明登记丢失前它是启用的；没有记录就只登记、不擅自加进输入法菜单。
    static func previouslyEnabled() -> Bool {
        guard let entries = UserDefaults(suiteName: "com.apple.inputsources")?
            .array(forKey: "AppleEnabledThirdPartyInputSources") else { return false }
        return entries.contains { entry in
            guard let fields = entry as? [String: Any] else { return false }
            return fields.values.contains { ($0 as? String)?.hasPrefix(parentID) == true }
        }
    }

    static func sources(id: String) -> [TISInputSource] {
        let filter = [kTISPropertyInputSourceID as String: id] as CFDictionary
        guard let list = TISCreateInputSourceList(filter, true)?.takeRetainedValue() as? [TISInputSource] else {
            return []
        }
        return list
    }

    static func isEnabled(_ source: TISInputSource) -> Bool {
        guard let raw = TISGetInputSourceProperty(source, kTISPropertyInputSourceIsEnabled) else { return false }
        return CFBooleanGetValue(Unmanaged<CFBoolean>.fromOpaque(raw).takeUnretainedValue())
    }
}
