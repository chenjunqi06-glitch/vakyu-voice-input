// 底部固定语音浮窗，字标一半露在卡片外；内外点击不触发取消。
import SwiftUI

struct VoicePanelView: View {
    @ObservedObject var model: VoiceModel

    let action: (String, Int?) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 12) {
                Text("Vakyu").font(.system(size: 16, weight: .bold, design: .rounded))
                MeterView(model: model.meter)
                Text(model.recording ? "正在聆听" : model.status)
                    .font(.system(size: 12)).foregroundStyle(.white.opacity(0.75))
                Spacer()
            }
            if let practice = model.practice {
                PracticeView(snapshot: practice, commitFailed: model.commitFailed, action: action)
            } else {
                ScrollView {
                    Text(model.dictation.isEmpty ? "按住 \(model.activeShortcutLabel) 说话，松开结束" : model.dictation)
                        .font(.system(size: 18)).frame(maxWidth: .infinity, alignment: .leading)
                        .fixedSize(horizontal: false, vertical: true)
                }.frame(maxHeight: 102)
                Text("只填入当前输入框 · 不自动发送 · Esc 取消")
                    .font(.caption).foregroundStyle(.white.opacity(0.6))
                if model.commitFailed {
                    Button("重新填入") { action("retry", nil) }.buttonStyle(.bordered)
                }
            }
            if !model.error.isEmpty {
                Text(model.error).font(.system(size: 12)).foregroundStyle(Color(red: 1, green: 0.87, blue: 0.65))
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .foregroundStyle(.white)
        .padding(.horizontal, 24).padding(.top, 23).padding(.bottom, 19)
        .background(Brand.gradient, in: RoundedRectangle(cornerRadius: 22))
        .overlay(RoundedRectangle(cornerRadius: 22).stroke(.white.opacity(0.18), lineWidth: 1))
        .overlay(alignment: .topLeading) {
            if let image = Brand.image("vakyu-logo.png") {
                Image(nsImage: image).resizable().scaledToFit().frame(width: 116, height: 58)
                    .offset(x: -10, y: -29).allowsHitTesting(false)
            }
        }
        .shadow(color: .black.opacity(0.24), radius: 16, x: 0, y: 7)
        .padding(.top, 38).padding(.horizontal, 24).padding(.bottom, 18)
    }
}
