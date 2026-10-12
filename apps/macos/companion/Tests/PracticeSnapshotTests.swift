// 校验 Rust 服务与 Swift 窗口的协议字段，防止中文拼音或词卡静默丢失。
import XCTest
@testable import VakyuVoice

final class PracticeSnapshotTests: XCTestCase {
    func testBilingualPracticeViewIncludesReadingAndDictionary() {
        let raw: [String: Any] = [
            "learn_english": false, "pronunciations": ["nǐ", "hǎo"],
            "original": "hello", "words": ["你", "好"], "matched_words": 1,
            "level_label": "新手 Lv.1", "level_progress": "0 / 10",
            "recognized": "你", "feedback": "继续", "actions_enabled": true,
            "recording": true, "status": "跟读中",
            "word_card": ["word": "你好", "meaning": "hello", "pronunciation": "nǐ hǎo"]
        ]
        let snapshot = PracticeSnapshot.decode(raw)
        XCTAssertNotNil(snapshot)
        XCTAssertEqual(snapshot?.pronunciations, ["nǐ", "hǎo"])
        XCTAssertEqual(snapshot?.matchedWords, 1)
        XCTAssertEqual(snapshot?.wordCard?.meaning, "hello")
    }

    func testConfigurationOnlyReadsDisplayWhitelist() {
        let config = VoiceConfiguration(["voice_input": true, "practice": true,
                                        "learn_english": false, "hold_ms": 100,
                                        "dictation_key": "left_alt",
                                        "shortcut": "right_ctrl+right_alt"])
        XCTAssertTrue(config.practice)
        XCTAssertFalse(config.learnEnglish)
        XCTAssertEqual(config.holdMilliseconds, 100)
        XCTAssertEqual(config.shortcut, "right_ctrl+right_alt")
        XCTAssertEqual(config.dictationKey, "left_alt")
    }
}
