// 底部固定语音浮窗，字标一半露在卡片外；内外点击不触发取消。
import SwiftUI

struct VoicePanelView: View {
    @ObservedObject var model: VoiceModel

    let action: (String, Int?) -> Void

    /// 启动前的错误单独展示，不建立录音或跟读会话。
    var notice: String? = nil

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            if let notice {
                Label("语音输入尚未开始", systemImage: "exclamationmark.circle")
                    .font(.system(size: 12, weight: .semibold)).foregroundStyle(.white.opacity(0.8))
                Text(notice)
                    .font(.system(size: 12))
                    .foregroundStyle(Color(red: 1, green: 0.87, blue: 0.65))
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .fixedSize(horizontal: false, vertical: true)
            } else {
                roundContent
            }
        }
        .foregroundStyle(.white)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .padding(.horizontal, 12).padding(.top, 12).padding(.bottom, 10)
        .background(Brand.gradient, in: RoundedRectangle(cornerRadius: 12))
        .overlay(RoundedRectangle(cornerRadius: 12).stroke(.white.opacity(0.18), lineWidth: 1))
        .overlay(alignment: .topLeading) {
            Group {
                if let image = Brand.image("vakyu-logo.png") {
                    Image(nsImage: image).resizable().scaledToFit()
                } else {
                    Text("Vakyu").font(.system(size: 15, weight: .bold, design: .rounded))
                        .foregroundStyle(.white)
                }
            }
            .frame(width: 58, height: 29)
            .offset(x: -5, y: -14.5).allowsHitTesting(false)
        }
        .shadow(color: .black.opacity(0.24), radius: 8, x: 0, y: 3.5)
        .padding(.top, 19).padding(.horizontal, 12).padding(.bottom, 9)
    }

    @ViewBuilder
    private var roundContent: some View {
        HStack(spacing: 6) {
            Label(modeTitle, systemImage: model.entryMode == .practice ? "text.book.closed" : "mic.fill")
                .font(.system(size: 12, weight: .semibold)).fixedSize()
            Spacer(minLength: 0)
            MeterView(model: model.meter, compact: true)
            Text(recordingStatus)
                .font(.system(size: 11)).foregroundStyle(.white.opacity(0.75))
                .lineLimit(1).help(recordingStatus)
        }
        if let practice = model.practice {
            PracticeView(snapshot: practice, commitFailed: model.commitFailed,
                         error: model.error, action: action, pastedWithoutVerification: model.pastedWithoutVerification)
        } else {
            ScrollViewReader { reader in
                ScrollView {
                    VStack(alignment: .leading, spacing: 5) {
                        if !model.error.isEmpty {
                            Text(model.error).font(.system(size: 12))
                                .foregroundStyle(Color(red: 1, green: 0.87, blue: 0.65))
                                .fixedSize(horizontal: false, vertical: true).id("dictation-error")
                        }
                        Text(model.dictation.isEmpty ? recordingPrompt : model.dictation)
                            .font(.system(size: 14))
                            .fixedSize(horizontal: false, vertical: true)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
                .onChange(of: model.error) { message in
                    if !message.isEmpty { reader.scrollTo("dictation-error", anchor: .top) }
                }
            }
            HStack(spacing: 6) {
                Text(footerText)
                    .font(.system(size: 10)).foregroundStyle(.white.opacity(0.65))
                    .fixedSize(horizontal: false, vertical: true)
                Spacer(minLength: 0)
                if model.commitFailed {
                    Button("复制文字") { action("copy-result", nil) }
                        .buttonStyle(.bordered).controlSize(.small)
                    Button("收起") { action("close-result", nil) }
                        .buttonStyle(.bordered).controlSize(.small)
                    if !model.pastedWithoutVerification {
                        Button("重新填入") { action("retry", nil) }
                            .buttonStyle(.bordered).controlSize(.small)
                    }
                }
            }
        }
    }

    private var learnEnglish: Bool { model.practice?.learnEnglish ?? model.configuration.learnEnglish }

    private var modeTitle: String {
        model.entryMode == .practice ? (learnEnglish ? "英语跟读" : "中文跟读") : "语音输入"
    }

    private var recordingStatus: String {
        guard model.recording else { return model.status }
        if model.entryMode == .dictation { return "正在聆听" }
        return model.practice == nil ? "正在录制原话" : "正在跟读"
    }

    private var recordingPrompt: String {
        guard model.entryMode == .practice else {
            return "按住 \(model.activeShortcutLabel) 说话，松开结束"
        }
        let language = learnEnglish ? "中文" : "英文"
        return "按住 \(model.activeShortcutLabel) 先说\(language)\n松开后开始\(modeTitle)"
    }

    private var footerText: String {
        if model.commitFailed { return "结果保留在菜单" }
        if model.entryMode == .practice { return "跟读后填入原话 · 不发送 · Esc 取消" }
        return "仅填入当前输入框 · 不发送 · Esc 取消"
    }
}
