// 波形柱来自近期真实 RMS 电平；静音时保持低位，不播放虚构振幅。
import SwiftUI

struct MeterView: View {
    @ObservedObject var model: AudioMeterState

    var body: some View {
        HStack(alignment: .center, spacing: 3) {
            ForEach(Array(model.values.enumerated()), id: \.offset) { _, level in
                Capsule().fill(Color.white.opacity(0.82))
                    .frame(width: 3, height: max(3, 27 * CGFloat(level)))
            }
        }
        .frame(height: 30)
        .animation(.linear(duration: 0.065), value: model.values)
        .accessibilityLabel("当前麦克风音量")
    }
}
