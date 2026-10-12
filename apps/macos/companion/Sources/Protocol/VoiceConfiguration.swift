// 服务只发送允许展示的设置，API 密钥永不进入 Swift 窗口或日志。
import Foundation

struct VoiceConfiguration {
    var enabled = true

    var pasteFallback = true

    var practice = false

    var learnEnglish = true

    var demo = true

    var shortcut = "right_ctrl+right_alt"

    var dictationKey = "left_alt"

    var holdMilliseconds = 100

    var similarity = 78

    var nameSimilarity = 45

    init() {}

    init(_ json: [String: Any]) {
        enabled = json["voice_input"] as? Bool ?? true
        pasteFallback = json["paste_fallback"] as? Bool ?? true
        practice = json["practice"] as? Bool ?? false
        learnEnglish = json["learn_english"] as? Bool ?? true
        demo = json["demo"] as? Bool ?? true
        shortcut = json["shortcut"] as? String ?? "right_ctrl+right_alt"
        dictationKey = json["dictation_key"] as? String ?? "left_alt"
        holdMilliseconds = max(0, min(1000, json["hold_ms"] as? Int ?? 100))
        similarity = json["similarity"] as? Int ?? 78
        nameSimilarity = json["name_similarity"] as? Int ?? 45
    }
}
