// 音量单独发布，避免每秒 30 次电平更新连带重排千字跟读正文。
import SwiftUI

final class AudioMeterState: ObservableObject {
    @Published private(set) var values: [Float] = Array(repeating: 0, count: 19)

    func append(_ level: Float) {
        values.removeFirst()
        values.append(max(0, min(1, level)))
    }

    func reset() { values = Array(repeating: 0, count: 19) }
}
