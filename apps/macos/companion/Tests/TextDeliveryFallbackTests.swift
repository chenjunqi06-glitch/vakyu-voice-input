//! 使用真实投递状态机回放一次性受限粘贴，不访问系统剪贴板或应用内容。
import XCTest
@testable import VakyuVoice

final class TextDeliveryFallbackTests: XCTestCase {
    func testFallbackPastesOnceWithoutClaimingVerifiedSuccess() {
        let access = TextDeliveryMemoryAccess(text: nil, range: nil)
        let delivery = TextDelivery(access: access, schedule: access.schedule)
        var target = access.target(web: true)
        target.fallbackWindowID = 7
        delivery.target = target
        var results: [Bool] = []
        var messages: [String] = []
        delivery.commit(id: 1, text: "test") { results.append($0); messages.append($1) }
        XCTAssertEqual(results, [false])
        XCTAssertTrue(messages[0].contains("已粘贴，无法核对"))
        XCTAssertTrue(delivery.pastedWithoutVerification)
        XCTAssertTrue(delivery.hasUnconfirmedWrite)
        XCTAssertEqual(delivery.failureStatus, "已粘贴，无法核对")
        delivery.commit(id: 1, text: "test") { results.append($0); _ = $1 }
        delivery.commit(id: 2, text: "test") { results.append($0); _ = $1 }
        access.runAll()
        XCTAssertEqual(access.pastes, 1)
        XCTAssertEqual(access.restorations, 1)
        XCTAssertTrue(access.writes.isEmpty)
        XCTAssertEqual(results, [false, false, false])
    }

    func testChangedContextBlocksFallbackBeforeClipboardOrKeyEvents() {
        let access = TextDeliveryMemoryAccess(text: nil, range: nil)
        let delivery = TextDelivery(access: access, schedule: access.schedule)
        var target = access.target(web: true)
        target.fallbackWindowID = 7
        delivery.target = target
        access.contextValid = false
        delivery.commit(id: 1, text: "test") { XCTAssertFalse($0); _ = $1 }
        XCTAssertEqual(access.pastes, 0)
        XCTAssertFalse(delivery.pastedWithoutVerification)
        XCTAssertFalse(delivery.hasUnconfirmedWrite)
    }

    func testResetClearsNoticeAndPreservesScheduledClipboardRestore() {
        let access = TextDeliveryMemoryAccess(text: nil, range: nil)
        let delivery = TextDelivery(access: access, schedule: access.schedule)
        var target = access.target(web: true)
        target.fallbackWindowID = 7
        delivery.target = target
        delivery.commit(id: 1, text: "test") { _, _ in }
        delivery.reset()
        access.runAll()
        XCTAssertFalse(delivery.pastedWithoutVerification)
        XCTAssertFalse(delivery.hasUnconfirmedWrite)
        XCTAssertEqual(access.restorations, 1)
        XCTAssertEqual(access.pastes, 1)
    }

    func testConfigurationDefaultsOnAndAcceptsExplicitOff() {
        XCTAssertTrue(VoiceConfiguration([:]).pasteFallback)
        XCTAssertFalse(VoiceConfiguration(["paste_fallback": false]).pasteFallback)
    }

    func testDisablingFallbackAfterCapturePreventsPaste() {
        let access = TextDeliveryMemoryAccess(text: nil, range: nil)
        let delivery = TextDelivery(access: access, schedule: access.schedule)
        var target = access.target(web: true)
        target.fallbackWindowID = 7
        delivery.target = target
        delivery.pasteFallbackEnabled = false
        delivery.commit(id: 1, text: "test") { XCTAssertFalse($0); _ = $1 }
        XCTAssertEqual(access.pastes, 0)
        XCTAssertTrue(access.writes.isEmpty)
        XCTAssertFalse(delivery.hasUnconfirmedWrite)
    }
}
