// 菜单栏图标的左右点击均显示主要开关与设置，菜单状态来自服务配置。
import AppKit

final class StatusItemController: NSObject {
    private let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)

    private weak var coordinator: VoiceCoordinator?

    init(coordinator: VoiceCoordinator) {
        self.coordinator = coordinator
        super.init()
        if let image = Brand.image("vakyu-menu.png") {
            image.size = NSSize(width: 18, height: 18); image.isTemplate = false
            item.button?.image = image
            item.button?.imagePosition = .imageOnly
            item.button?.imageScaling = .scaleProportionallyDown
        } else { item.button?.title = "V" }
        item.button?.toolTip = "Vakyu 语音输入"
        refresh()
    }

    func refresh() {
        guard let coordinator else { return }
        let config = coordinator.model.configuration
        let menu = NSMenu()
        let title = NSMenuItem(title: "Vakyu · \(coordinator.model.availabilityMessage)",
                               action: nil, keyEquivalent: "")
        item.button?.toolTip = "Vakyu：\(coordinator.model.availabilityMessage)"
        title.isEnabled = false; menu.addItem(title); menu.addItem(.separator())
        add(menu, "启用语音输入", #selector(toggleVoice), checked: config.enabled)
        add(menu, "跟读（读完填入原话）", #selector(toggleShadowing),
            checked: config.enabled && config.practice && !config.sendTranslation)
        add(menu, config.learnEnglish ? "说中发英（读完填入英文）" : "说英发中（读完填入中文）",
            #selector(toggleSendTranslation), checked: config.enabled && config.practice && config.sendTranslation)
        add(menu, "学习英语（关闭后学习中文）", #selector(toggleDirection), checked: config.learnEnglish)
        add(menu, "播放整句示范", #selector(toggleDemo), checked: config.demo)
        menu.addItem(.separator())
        if !coordinator.model.lastResult.isEmpty {
            add(menu, "复制上次识别文字", #selector(copyResult))
            if coordinator.model.visible && coordinator.model.commitFailed {
                add(menu, "查看上次填入结果…", #selector(reopenResult))
            }
            menu.addItem(.separator())
        }
        add(menu, "显示快捷浮窗", #selector(toggleQuickBar), checked: coordinator.quickBar.isShown)
        add(menu, "语音设置…", #selector(openSettings))
        add(menu, "输入法详细设置…", #selector(openInputSettings))
        menu.addItem(.separator())
        add(menu, "退出语音服务", #selector(quit))
        item.menu = menu
    }

    private func add(_ menu: NSMenu, _ title: String, _ action: Selector, checked: Bool? = nil) {
        let entry = NSMenuItem(title: title, action: action, keyEquivalent: "")
        entry.target = self
        if let checked { entry.state = checked ? .on : .off }
        menu.addItem(entry)
    }

    func showNotice(_ message: String) { item.button?.toolTip = "Vakyu：\(message)" }

    @objc private func toggleVoice() {
        guard let coordinator else { return }
        coordinator.set("voice_input", value: !coordinator.model.configuration.enabled)
    }

    @objc private func toggleShadowing() { coordinator?.togglePracticeOutput(sendTranslation: false) }

    @objc private func toggleSendTranslation() { coordinator?.togglePracticeOutput(sendTranslation: true) }

    @objc private func toggleQuickBar() {
        guard let coordinator else { return }
        coordinator.quickBar.setShown(!coordinator.quickBar.isShown)
        refresh()
    }

    @objc private func toggleDirection() {
        guard let coordinator else { return }
        coordinator.set("voice_practice_learn_english", value: !coordinator.model.configuration.learnEnglish)
    }

    @objc private func toggleDemo() {
        guard let coordinator else { return }
        coordinator.set("voice_practice_demo", value: !coordinator.model.configuration.demo)
    }

    @objc private func openSettings() { coordinator?.showSettings() }

    @objc private func copyResult() { coordinator?.copyLastResult() }

    @objc private func reopenResult() { coordinator?.reopenFailedResult() }

    @objc private func openInputSettings() { coordinator?.openInputSettings() }

    @objc private func quit() { NSApp.terminate(nil) }
}
