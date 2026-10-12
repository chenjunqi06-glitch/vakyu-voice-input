// 已读词只响应匹配进度做透明度过渡，音频事件不会重新构建整句匹配。
import SwiftUI

struct PracticeView: View {
    let snapshot: PracticeSnapshot

    let commitFailed: Bool

    var error = ""

    let action: (String, Int?) -> Void

    var pastedWithoutVerification = false

    var body: some View {
        VStack(alignment: .leading, spacing: 5) {
            HStack {
                Text(snapshot.levelLabel).font(.system(size: 10, weight: .semibold))
                Spacer()
                Text(snapshot.levelProgress).font(.system(size: 10)).foregroundStyle(.white.opacity(0.65))
            }
            ScrollViewReader { reader in
                ScrollView {
                    VStack(alignment: .leading, spacing: 6) {
                        Text("原话：\(snapshot.original)").font(.system(size: 11))
                            .foregroundStyle(.white.opacity(0.75))
                            .fixedSize(horizontal: false, vertical: true)
                        WordFlow(spacing: snapshot.learnEnglish ? 4 : 2) {
                            ForEach(snapshot.words.indices, id: \.self) { index in
                                Button { action("lookup", index) } label: {
                                    VStack(spacing: 2) {
                                        if index < snapshot.pronunciations.count, !snapshot.pronunciations[index].isEmpty {
                                            Text(snapshot.pronunciations[index]).font(.system(size: 10))
                                        }
                                        Text(snapshot.words[index]).font(.system(size: 17, weight: .semibold))
                                    }
                                    .fixedSize(horizontal: false, vertical: true)
                                    .padding(.horizontal, 3).padding(.vertical, 3)
                                    .foregroundStyle(.white.opacity(index < snapshot.matchedWords ? 1 : 0.42))
                                    .background(index == snapshot.matchedWords ? Color.white.opacity(0.10) : .clear,
                                                in: RoundedRectangle(cornerRadius: 5))
                                }
                                .buttonStyle(.plain).id(index)
                                .animation(.easeInOut(duration: 0.18), value: snapshot.matchedWords)
                                .help("查看词义与发音")
                            }
                        }
                        if let card = snapshot.wordCard {
                            WordCardView(card: card, action: action).id("word-card")
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
                .onAppear {
                    if snapshot.wordCard != nil {
                        reader.scrollTo("word-card", anchor: .top)
                    } else {
                        reader.scrollTo(min(snapshot.matchedWords, max(0, snapshot.words.count - 1)), anchor: .top)
                    }
                }
                .onChange(of: snapshot.matchedWords) { count in
                    guard snapshot.wordCard == nil else { return }
                    withAnimation(.easeOut(duration: 0.18)) {
                        reader.scrollTo(min(count, max(0, snapshot.words.count - 1)), anchor: .center)
                    }
                }
                .onChange(of: snapshot.wordCard?.word) { word in
                    if word != nil {
                        reader.scrollTo("word-card", anchor: .top)
                    } else {
                        reader.scrollTo(min(snapshot.matchedWords, max(0, snapshot.words.count - 1)), anchor: .center)
                    }
                }
            }
            ScrollViewReader { reader in
                ScrollView {
                    VStack(alignment: .leading, spacing: 2) {
                        if !error.isEmpty {
                            Text(error).foregroundStyle(Color(red: 1, green: 0.87, blue: 0.65))
                                .fixedSize(horizontal: false, vertical: true).id("practice-error")
                        }
                        Text(snapshot.feedback.isEmpty ? snapshot.status : snapshot.feedback)
                            .foregroundStyle(.white.opacity(0.9))
                            .fixedSize(horizontal: false, vertical: true)
                        Text(snapshot.recognized.isEmpty ? "刚听到：等待朗读…" : "刚听到：\(snapshot.recognized)")
                            .foregroundStyle(.white.opacity(0.7))
                            .fixedSize(horizontal: false, vertical: true)
                    }
                    .font(.system(size: 11))
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
                .onChange(of: error) { message in
                    if !message.isEmpty { reader.scrollTo("practice-error", anchor: .top) }
                }
            }.frame(height: 30)
            HStack(spacing: 5) {
                if !pastedWithoutVerification {
                    Button(commitFailed ? "重新填入" : "再试一次") { action("retry", nil) }
                }
                if commitFailed {
                    Button("复制") { action("copy-result", nil) }
                    Button("收起") { action("close-result", nil) }
                } else {
                    Button("跳过此词") { action("skip", nil) }.disabled(snapshot.words.isEmpty)
                    Button("我读对了") { action("accept", nil) }.disabled(snapshot.words.isEmpty)
                }
                Spacer(minLength: 0)
                Text("Esc 取消").font(.system(size: 10)).foregroundStyle(.white.opacity(0.6))
            }
            .font(.system(size: 11)).controlSize(.small)
            .buttonStyle(.bordered).tint(.white.opacity(0.15)).fixedSize(horizontal: false, vertical: true)
            .disabled(!snapshot.actionsEnabled && !commitFailed)
        }
    }
}
