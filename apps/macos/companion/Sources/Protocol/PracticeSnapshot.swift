// 共享服务发来的完整练习快照；Swift 不复制或改写跟读判定。
import Foundation

struct PracticeSnapshot: Decodable, Equatable {
    let learnEnglish: Bool

    let pronunciations: [String]

    let original: String

    let words: [String]

    let matchedWords: Int

    let levelLabel: String

    let levelProgress: String

    let recognized: String

    let feedback: String

    let actionsEnabled: Bool

    let recording: Bool

    let wordCard: WordCard?

    let status: String

    static func decode(_ object: Any) -> PracticeSnapshot? {
        guard let data = try? JSONSerialization.data(withJSONObject: object) else { return nil }
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        return try? decoder.decode(Self.self, from: data)
    }
}
