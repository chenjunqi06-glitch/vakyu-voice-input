// 波形柱来自近期真实 RMS 电平；静音时保持低位，不播放虚构振幅。
import SwiftUI

struct MeterView: View {
    @ObservedObject var model: AudioMeterState

    var compact = false

    var body: some View {
        HStack(alignment: .center, spacing: compact ? 1 : 3) {
            ForEach(Array(model.values.enumerated()), id: \.offset) { _, level in
                Capsule().fill(Color.white.opacity(0.82))
                    .frame(width: compact ? 2 : 3,
                           height: max(compact ? 2 : 3, (compact ? 14 : 27) * CGFloat(level)))
            }
        }
        .frame(height: compact ? 16 : 30)
        .animation(.linear(duration: 0.065), value: model.values)
        .accessibilityLabel("当前麦克风音量")
    }
}
