// 点击词条只显示本地释义；播放动作交回共享服务统一暂停和恢复识别。
import SwiftUI

struct WordCardView: View {
    let card: WordCard

    let action: (String, Int?) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 7) {
            HStack {
                Text(card.word).font(.headline)
                if let pronunciation = card.pronunciation { Text(pronunciation).foregroundStyle(.white.opacity(0.75)) }
                Spacer()
                Button { action("speak_word", nil) } label: { Image(systemName: "speaker.wave.2.fill") }
                    .help("播放本机发音")
                Button { action("close_card", nil) } label: { Image(systemName: "xmark") }
                    .help("收起词卡")
            }
            Text(card.meaning ?? "本地词典暂未收录这个词。")
                .font(.system(size: 13)).fixedSize(horizontal: false, vertical: true)
            if let hint = card.hint { Text(hint).font(.caption).foregroundStyle(.white.opacity(0.65)) }
        }
        .padding(12)
        .background(Color.white.opacity(0.10), in: RoundedRectangle(cornerRadius: 12))
        .buttonStyle(.plain)
    }
}
