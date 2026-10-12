// 按顺序引导系统授权，提供当前应用的拖入入口，避免用户查找隐藏的安装目录。
import AppKit
import SwiftUI

struct PermissionSetupView: View {
    let coordinator: VoiceCoordinator

    @ObservedObject var model: VoiceModel

    @State private var pathCopied = false

    private var grantedCount: Int {
        [model.microphonePermission, model.accessibilityPermission]
            .filter { $0 }.count
    }

    var body: some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 14) {
                Text(model.permissionsReady ? "语音权限与快捷键已就绪" : "已完成 \(grantedCount)/2 项授权，点击下方按钮继续")
                    .font(.callout.weight(.semibold))
                permissionRow("1  麦克风", detail: "用于按住快捷键时录音。",
                              allowed: model.microphonePermission, actionLabel: "允许麦克风") {
                    PermissionState.requestMicrophone { coordinator.refreshPermissions() }
                }
                permissionRow("2  辅助功能", detail: "用于把文字填入你选中的输入框。",
                              allowed: model.accessibilityPermission, actionLabel: "打开辅助功能") {
                    PermissionState.requestAccessibility()
                }
                if model.accessibilityMayBeStale {
                    Text(PermissionState.staleAccessibilityMessage)
                        .font(.caption).foregroundStyle(.yellow)
                    Button("打开系统辅助功能设置") { PermissionState.open("Accessibility") }
                }
                Label(model.keyboardMonitorAvailable ? "全局快捷键已连接" : "全局快捷键尚未连接",
                      systemImage: model.keyboardMonitorAvailable ? "checkmark.circle.fill" : "keyboard")
                    .font(.caption).foregroundStyle(.white.opacity(0.8))
                if model.accessibilityPermission && !model.keyboardMonitorAvailable {
                    permissionRow("输入监控", detail: "若系统仍未允许连接快捷键，请开启后重试。",
                                  allowed: model.keyboardPermission, actionLabel: "打开输入监控") {
                        PermissionState.requestKeyboard()
                    }
                }
                if !model.accessibilityPermission || !model.keyboardMonitorAvailable || model.accessibilityMayBeStale {
                    Divider().overlay(.white.opacity(0.15))
                    Text("在打开的系统页面中，打开 Vakyu 的开关。如果列表中没有它，把下面的应用拖入列表即可。")
                        .font(.caption).foregroundStyle(.white.opacity(0.8))
                    applicationCard
                    DisclosureGroup("开关已打开，但这里仍未通过？") {
                        Text("先按系统提示选择“退出并重新打开”。若仍未通过，在对应权限列表中删除旧的 Vakyu 项，再拖入上方的当前应用并打开开关。")
                            .font(.caption).foregroundStyle(.white.opacity(0.8)).padding(.top, 6)
                    }.font(.caption)
                }
                HStack {
                    Text("允许后会自动检查；系统要求退出并重新打开时，请按提示完成。")
                        .font(.caption).foregroundStyle(.white.opacity(0.65))
                    Spacer()
                    Button("重新检查") { coordinator.refreshPermissions() }
                }
            }.padding(8)
        } label: { Label("系统权限", systemImage: "lock.shield.fill") }
    }

    private var applicationCard: some View {
        HStack(spacing: 12) {
            HStack(spacing: 10) {
                Image(nsImage: Brand.image("vakyu-icon.png")
                      ?? NSWorkspace.shared.icon(forFile: PermissionState.applicationURL.path))
                    .resizable().scaledToFit().frame(width: 38, height: 38)
                VStack(alignment: .leading, spacing: 4) {
                    Text("Vakyu 语音应用").font(.callout.weight(.semibold))
                    Label("拖到系统设置的权限列表", systemImage: "hand.draw")
                        .font(.caption).foregroundStyle(.white.opacity(0.75))
                }
            }
            .padding(10)
            .background(.white.opacity(0.08), in: RoundedRectangle(cornerRadius: 8))
            .onDrag { NSItemProvider(object: PermissionState.applicationURL as NSURL) }
            .accessibilityHint("将当前应用拖到辅助功能或输入监控列表")
            Spacer()
            VStack(alignment: .trailing, spacing: 8) {
                Button("在访达中显示") { PermissionState.revealApplication() }
                Button(pathCopied ? "位置已复制" : "复制应用位置") {
                    PermissionState.copyApplicationPath()
                    pathCopied = true
                }
            }
        }
    }

    private func permissionRow(_ title: String, detail: String, allowed: Bool,
                               actionLabel: String, action: @escaping () -> Void) -> some View {
        HStack(spacing: 12) {
            Image(systemName: allowed ? "checkmark.circle.fill" : "circle")
                .foregroundStyle(allowed ? Color.green : .white.opacity(0.55))
            VStack(alignment: .leading, spacing: 3) {
                Text(title).font(.callout.weight(.medium))
                Text(detail).font(.caption).foregroundStyle(.white.opacity(0.7))
            }
            Spacer()
            Button(allowed ? "查看设置" : actionLabel, action: action)
        }
    }
}
