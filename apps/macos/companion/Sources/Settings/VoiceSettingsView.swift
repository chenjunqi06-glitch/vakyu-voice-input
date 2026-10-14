// 语音与双向学习的用户设置；翻译密钥沿用主输入法设置，不复制保存。
import SwiftUI
import AVFoundation

struct VoiceSettingsView: View {
    let coordinator: VoiceCoordinator

    @ObservedObject var model: VoiceModel

    @AppStorage("voice.englishIdentifier") private var englishVoice = ""

    @AppStorage("voice.chineseIdentifier") private var chineseVoice = ""

    @AppStorage(QuickBarPanel.visibleKey) private var quickBarVisible = true

    private var practiceFlow: String {
        let config = model.configuration
        switch (config.learnEnglish, config.sendTranslation) {
        case (true, false): return "说中文 → 跟读英文 → 填入原中文"
        case (true, true): return "说中文 → 跟读英文 → 填入英文"
        case (false, false): return "说英文 → 跟读中文（含拼音）→ 填入原英文"
        case (false, true): return "说英文 → 跟读中文（含拼音）→ 填入中文"
        }
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 22) {
                header
                GroupBox {
                    VStack(alignment: .leading, spacing: 14) {
                        Toggle("启用全局语音输入", isOn: toggle("voice_input", \.enabled))
                        Toggle("启用口语跟读快捷键", isOn: toggle("voice_english_practice", \.practice))
                        Toggle("学习英语", isOn: toggle("voice_practice_learn_english", \.learnEnglish))
                        Toggle(model.configuration.learnEnglish ? "读完发出英文" : "读完发出中文",
                               isOn: toggle("voice_practice_send_translation", \.sendTranslation))
                        Text(practiceFlow)
                            .font(.caption).foregroundStyle(.white.opacity(0.72))
                        Toggle("跟读与划词用云端 API 翻译", isOn: Binding(
                            get: { model.configuration.translationEngine == "cloud" },
                            set: { coordinator.set("translation_engine", value: $0 ? "cloud" : "local") }))
                        Text("关闭（默认）时用 macOS 系统翻译：完全在本机运行，免费，需要 macOS 15 及以上；第一次使用前在“系统设置 → 通用 → 语言与地区 → 翻译语言”下载中文（简体）和英语。打开后改用“云服务”里的接口，句子更自然，按服务商价格计费。")
                            .font(.caption).foregroundStyle(.white.opacity(0.65))
                        Toggle("显示快捷浮窗（一键开关跟读 / 发英）", isOn: $quickBarVisible)
                            .onChange(of: quickBarVisible) { _ in
                                coordinator.quickBar.restore(); coordinator.statusItem.refresh()
                            }
                        Toggle("跟读前播放整句示范", isOn: toggle("voice_practice_demo", \.demo))
                        Text("普通语音始终直接填入，不经过跟读。口语跟读使用另一个快捷键，关闭后该快捷键不触发录音。两种方式都不自动发送；Esc 可取消。只有口语跟读为每次原话请求一次翻译。")
                            .font(.caption).foregroundStyle(.white.opacity(0.65))
                    }.padding(8)
                } label: { Label("语音与学习方向", systemImage: "mic.fill") }
                shortcutSection
                permissionSection
                GroupBox {
                    VStack(alignment: .leading, spacing: 12) {
                        voicePicker("英语示范音色", selection: $englishVoice, language: "en")
                        voicePicker("中文示范音色", selection: $chineseVoice, language: "zh")
                        Text("使用 macOS 本机声音，不耗 API token，也不需要独立显卡。更多声音可在系统设置的“朗读内容”中下载。")
                            .font(.caption).foregroundStyle(.white.opacity(0.65))
                    }.padding(8)
                } label: { Label("示范朗读", systemImage: "speaker.wave.2.fill") }
                GroupBox {
                    VStack(alignment: .leading, spacing: 10) {
                        threshold("普通词最低相似度", key: "voice_practice_similarity", value: model.configuration.similarity)
                        threshold("人名与专名最低相似度", key: "voice_practice_name_similarity", value: model.configuration.nameSimilarity)
                        Text("数值越低越宽容。新手阶段使用该阈值，随本机练习经验逐步提高要求。遇到识别偏差可用“我读对了”或“跳过此词”，不会卡死。")
                            .font(.caption).foregroundStyle(.white.opacity(0.65))
                    }.padding(8)
                } label: { Label("发音容错", systemImage: "person.wave.2.fill") }
                HStack {
                    Button("输入法详细设置") { coordinator.openInputSettings() }
                    Spacer()
                    Button("重新启动语音服务") { coordinator.restart() }
                        .disabled(model.visible)
                }
                if !model.settingMessage.isEmpty {
                    Text(model.settingMessage).font(.caption).foregroundStyle(.white.opacity(0.8))
                }
                Text("Vakyu · 本机语音与离线词典 · Apple Silicon")
                    .font(.caption2).foregroundStyle(.white.opacity(0.45))
            }
            .padding(.horizontal, 30).padding(.top, 44).padding(.bottom, 28)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .foregroundStyle(.white)
        .background(Brand.gradient)
        .tint(Color(red: 0.78, green: 0.63, blue: 1))
        .onAppear { coordinator.refreshPermissions() }
    }

    private var header: some View {
        HStack(spacing: 18) {
            if let image = Brand.image("vakyu-logo.png") {
                Image(nsImage: image).resizable().scaledToFit().frame(width: 140, height: 72)
            }
            VStack(alignment: .leading, spacing: 6) {
                Text("语音设置").font(.system(size: 26, weight: .bold))
                Label(model.ready ? "语音模型就绪" : model.status,
                      systemImage: model.ready ? "checkmark.circle.fill" : "hourglass")
                    .font(.caption).foregroundStyle(.white.opacity(0.7))
            }
            Spacer()
        }
    }

    private var shortcutSection: some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 12) {
                HStack {
                    Text("普通语音输入")
                    Spacer()
                    Text(model.dictationShortcutLabel).fontWeight(.semibold)
                }
                Text("按住普通语音键说话，松开即填入原话，不进入跟读。")
                    .font(.caption).foregroundStyle(.white.opacity(0.7))
                Divider().overlay(.white.opacity(0.15))
                HStack {
                    Text("口语跟读")
                    Text(model.shortcutRecording ? "请按下新的快捷键…" : model.shortcutLabel)
                        .font(.system(size: 15, weight: .semibold))
                    Spacer()
                    Button("修改跟读键") { coordinator.recordShortcut() }.disabled(model.visible || model.shortcutRecording)
                    Button("恢复默认") { coordinator.saveShortcut(.defaultValue) }.disabled(model.visible)
                }
                Text("跟读默认右 Control＋右 Option。可改为 F8、Control＋Option＋E 等；不能与普通语音键相同，也不允许单独字母。两种入口切换前请完全松开上一组按键。")
                    .font(.caption).foregroundStyle(.white.opacity(0.7))
                if !model.shortcutMessage.isEmpty { Text(model.shortcutMessage).font(.caption) }
                Text("保存会检测系统注册的组合键占用。纯修饰键、硬件功能键以及只在某个应用内部生效的快捷键无法完整检测。")
                    .font(.caption).foregroundStyle(.white.opacity(0.6))
                HStack {
                    Text("按住启动延迟")
                    Spacer()
                    Picker("按住启动延迟", selection: Binding(
                        get: { model.configuration.holdMilliseconds },
                        set: { coordinator.set("voice_hold_ms", value: $0) })) {
                            Text("100 毫秒").tag(100)
                            Text("200 毫秒").tag(200)
                            Text("300 毫秒").tag(300)
                    }.labelsHidden().frame(width: 150)
                }.font(.caption)
            }.padding(8)
        } label: { Label("全局快捷键", systemImage: "keyboard") }
    }

    private var permissionSection: some View {
        GroupBox {
            VStack(spacing: 10) {
                permission("麦克风", allowed: model.microphonePermission) {
                    PermissionState.requestMicrophone { coordinator.refreshPermissions() }
                }
                permission("辅助功能（填入文字）", allowed: model.accessibilityPermission) { PermissionState.requestAccessibility() }
                permission("输入监控（全局按键）", allowed: model.keyboardPermission) { PermissionState.requestKeyboard() }
                HStack {
                    Text("首次安装需要允许系统权限。返回这里点刷新即可重新连接。")
                        .font(.caption).foregroundStyle(.white.opacity(0.65))
                    Spacer()
                    Button("刷新权限") { coordinator.refreshPermissions() }
                }
            }.padding(8)
        } label: { Label("系统权限", systemImage: "lock.shield.fill") }
    }

    private func toggle(_ key: String, _ path: KeyPath<VoiceConfiguration, Bool>) -> Binding<Bool> {
        Binding(get: { model.configuration[keyPath: path] }, set: { coordinator.set(key, value: $0) })
    }

    private func permission(_ label: String, allowed: Bool, request: @escaping () -> Void) -> some View {
        HStack {
            Label(label, systemImage: allowed ? "checkmark.circle.fill" : "exclamationmark.circle")
            Spacer()
            Button(allowed ? "系统设置" : "允许") { request() }
        }
    }

    private func voicePicker(_ label: String, selection: Binding<String>, language: String) -> some View {
        Picker(label, selection: selection) {
            Text("系统默认").tag("")
            ForEach(SpeechPlayer.voices(language: language), id: \.identifier) { voice in
                Text("\(voice.name) · \(voice.language)").tag(voice.identifier)
            }
        }
    }

    private func threshold(_ label: String, key: String, value: Int) -> some View {
        Stepper(value: Binding(get: { value }, set: { coordinator.set(key, value: $0) }), in: 30...95, step: 1) {
            Text("\(label)：\(value)%").font(.system(size: 13))
        }
    }
}
