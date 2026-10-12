// 点击词条只显示本地释义；播放动作交回共享服务统一暂停和恢复识别。
import SwiftUI

struct WordCardView: View {
    let card: WordCard

    let action: (String, Int?) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 5) {
            HStack(alignment: .top, spacing: 6) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(card.word).font(.system(size: 13, weight: .semibold))
                    if let pronunciation = card.pronunciation {
                        Text(pronunciation).font(.system(size: 11)).foregroundStyle(.white.opacity(0.75))
                    }
                }
                .fixedSize(horizontal: false, vertical: true)
                .frame(maxWidth: .infinity, alignment: .leading)
                Button { action("speak_word", nil) } label: { Image(systemName: "speaker.wave.2.fill") }
                    .frame(width: 20, height: 20)
                    .help("播放本机发音")
                Button { action("close_card", nil) } label: { Image(systemName: "xmark") }
                    .frame(width: 20, height: 20)
                    .help("收起词卡")
            }
            Text(card.meaning ?? "本地词典暂未收录这个词。")
                .font(.system(size: 12)).fixedSize(horizontal: false, vertical: true)
            if let hint = card.hint {
                Text(hint).font(.system(size: 10)).foregroundStyle(.white.opacity(0.65))
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .padding(8)
        .background(Color.white.opacity(0.10), in: RoundedRectangle(cornerRadius: 8))
        .buttonStyle(.plain)
    }
}
