// 已读词只响应匹配进度做透明度过渡，音频事件不会重新构建整句匹配。
import SwiftUI

struct PracticeView: View {
    let snapshot: PracticeSnapshot

    let commitFailed: Bool

    let action: (String, Int?) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text(snapshot.learnEnglish ? "英语跟读" : "中文跟读").font(.headline)
                Spacer()
                Text(snapshot.levelLabel).font(.caption.weight(.semibold))
                Text(snapshot.levelProgress).font(.caption).foregroundStyle(.white.opacity(0.65))
            }
            ScrollView { Text(snapshot.original).font(.system(size: 13)).frame(maxWidth: .infinity, alignment: .leading) }
                .frame(maxHeight: 46).foregroundStyle(.white.opacity(0.75))
            ScrollViewReader { reader in
                ScrollView {
                    WordFlow(spacing: snapshot.learnEnglish ? 5 : 2) {
                        ForEach(snapshot.words.indices, id: \.self) { index in
                            Button { action("lookup", index) } label: {
                                VStack(spacing: 2) {
                                    if index < snapshot.pronunciations.count, !snapshot.pronunciations[index].isEmpty {
                                        Text(snapshot.pronunciations[index]).font(.system(size: 11))
                                    }
                                    Text(snapshot.words[index]).font(.system(size: 21, weight: .semibold))
                                }
                                .padding(.horizontal, 3).padding(.vertical, 4)
                                .foregroundStyle(.white.opacity(index < snapshot.matchedWords ? 1 : 0.42))
                                .background(index == snapshot.matchedWords ? Color.white.opacity(0.10) : .clear,
                                            in: RoundedRectangle(cornerRadius: 6))
                            }
                            .buttonStyle(.plain).id(index)
                            .animation(.easeInOut(duration: 0.18), value: snapshot.matchedWords)
                            .help("查看词义与发音")
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
                .frame(maxHeight: snapshot.wordCard == nil ? 172 : 98)
                .onChange(of: snapshot.matchedWords) { count in
                    withAnimation(.easeOut(duration: 0.18)) {
                        reader.scrollTo(min(count, max(0, snapshot.words.count - 1)), anchor: .center)
                    }
                }
            }
            if let card = snapshot.wordCard { WordCardView(card: card, action: action) }
            VStack(alignment: .leading, spacing: 4) {
                Text(snapshot.recognized.isEmpty ? "刚听到：等待朗读…" : "刚听到：\(snapshot.recognized)")
                    .lineLimit(2).font(.system(size: 12)).foregroundStyle(.white.opacity(0.7))
                Text(snapshot.feedback.isEmpty ? snapshot.status : snapshot.feedback)
                    .font(.system(size: 12)).foregroundStyle(.white.opacity(0.9)).lineLimit(2)
            }
            VStack(alignment: .trailing, spacing: 5) {
                ViewThatFits(in: .horizontal) {
                    HStack(spacing: 8) { actionButtons }
                    LazyVGrid(columns: [GridItem(.flexible()), GridItem(.flexible())], spacing: 8) {
                        actionButtons
                    }
                }
                Text(snapshot.learnEnglish ? "Esc 取消" : "Esc Cancel")
                    .font(.caption).foregroundStyle(.white.opacity(0.6))
            }
            .font(.system(size: 12))
            .buttonStyle(.bordered).tint(.white.opacity(0.15))
            .disabled(!snapshot.actionsEnabled && !commitFailed)
        }
    }

    @ViewBuilder private var actionButtons: some View {
        Button(commitFailed
               ? (snapshot.learnEnglish ? "重新填入" : "Insert again")
               : (snapshot.learnEnglish ? "继续补读" : "Continue")) { action("retry", nil) }
            .help(snapshot.learnEnglish ? "保留已通过的词，从未通过处继续" : "Keep passed words and continue from the next word")
        Button(snapshot.learnEnglish ? "跳过此词" : "Skip") { action("skip", nil) }
            .disabled(commitFailed || snapshot.words.isEmpty)
        Button(snapshot.learnEnglish ? "我读对了" : "Accept") { action("accept", nil) }
            .disabled(commitFailed || snapshot.words.isEmpty)
        Button(snapshot.learnEnglish ? "整句重读" : "Restart") { action("restart", nil) }
            .disabled(commitFailed || snapshot.words.isEmpty)
            .help(snapshot.learnEnglish ? "清除本句已通过进度，从头练习" : "Clear this sentence's progress and read it from the start")
    }
}
