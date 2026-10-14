// 服务下发的配置：新增的「读完发出译文」缺省关闭，旧服务不发该字段时保持原行为。
import XCTest
@testable import VakyuVoice

final class VoiceConfigurationTests: XCTestCase {
    func testSendTranslationDefaultsOffForOlderServices() {
        XCTAssertFalse(VoiceConfiguration(["practice": true]).sendTranslation)
        XCTAssertTrue(VoiceConfiguration(["practice": true, "send_translation": true]).sendTranslation)
    }

    func testTranslationTitleFollowsTarget() {
        let state = TranslationState()
        XCTAssertEqual(state.title, "Vakyu 翻译中…")
        state.phase = .ready
        state.target = "zh"
        XCTAssertEqual(state.title, "Vakyu · 译成中文")
    }

    func testReadingTargetMatchesServiceRule() {
        XCTAssertEqual(SystemTranslator.readingTarget("今天下午开会"), "en")
        XCTAssertEqual(SystemTranslator.readingTarget("Meeting at 3 pm"), "zh")
        XCTAssertEqual(SystemTranslator.readingTarget("我在用 Vakyu 输入法"), "en")
    }
}
