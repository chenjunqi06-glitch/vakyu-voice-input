//! 分段精确确认与剪贴板恢复时限；所有控件和按键效果均由内存替身模拟。
import XCTest
@testable import VakyuVoice

final class TextDeliveryTimingTests: XCTestCase {
    func testExactPasteConfirmsEarlyButRestoresClipboardOnlyAfter750ms() {
        let access = TextDeliveryMemoryAccess()
        let clock = TextDeliveryTimingClock()
        let delivery = TextDelivery(access: access, schedule: clock.schedule)
        delivery.target = access.target(web: true)
        var results: [Bool] = []
        var reads = 0
        access.snapshotEffect = { _ in reads += 1 }
        delivery.commit(id: 1, text: "x") { results.append($0); _ = $1 }
        clock.advance(to: 0.049)
        XCTAssertTrue(results.isEmpty)
        clock.advance(to: 0.05)
        XCTAssertEqual(results, [true])
        XCTAssertEqual(reads, 1)
        XCTAssertEqual(access.restorations, 0)
        clock.advance(to: 0.35)
        XCTAssertEqual(results, [true])
        XCTAssertEqual(reads, 1)
        clock.advance(to: 0.749)
        XCTAssertEqual(access.restorations, 0)
        clock.advance(to: 0.75)
        XCTAssertEqual(access.restorations, 1)
        XCTAssertEqual(access.pastes, 1)
        XCTAssertTrue(access.writes.isEmpty)
    }

    func testSlowPasteKeeps350msDeadlineWithoutResendingInput() {
        let access = TextDeliveryMemoryAccess()
        access.pasteEffect = { _, _ in }
        let clock = TextDeliveryTimingClock()
        let delivery = TextDelivery(access: access, schedule: clock.schedule)
        delivery.target = access.target(web: true)
        var results: [Bool] = []
        delivery.commit(id: 1, text: "x") { results.append($0); _ = $1 }
        clock.advance(to: 0.05)
        XCTAssertTrue(results.isEmpty)
        delivery.commit(id: 1, text: "x") { results.append($0); _ = $1 }
        XCTAssertEqual(results, [false])
        clock.advance(to: 0.34)
        access.insert("x")
        clock.advance(to: 0.35)
        XCTAssertEqual(results, [false, true])
        XCTAssertEqual(access.pastes, 1)
        XCTAssertTrue(access.writes.isEmpty)
        XCTAssertEqual(access.restorations, 0)
        clock.advance(to: 0.75)
        XCTAssertEqual(access.restorations, 1)
    }

    func testTransientUnreadableOrPartialTextCanSettleBeforeOriginalDeadline() {
        for temporary in ["partial", nil] {
            let access = TextDeliveryMemoryAccess()
            access.pasteEffect = { memory, _ in
                memory.current = SelectionSnapshot(text: temporary, range: nil)
            }
            let clock = TextDeliveryTimingClock()
            let delivery = TextDelivery(access: access, schedule: clock.schedule)
            delivery.target = access.target(web: true)
            var result: Bool?
            delivery.commit(id: 1, text: "x") { result = $0; _ = $1 }
            clock.advance(to: 0.05)
            XCTAssertNil(result)
            access.current = SelectionSnapshot(text: "draftx", range: nil)
            clock.advance(to: 0.35)
            XCTAssertEqual(result, true)
            XCTAssertEqual(access.pastes, 1)
        }
    }

    func testUnconfirmedFinalReadLocksRetryEvenWhenTextAppearsLater() {
        let access = TextDeliveryMemoryAccess()
        access.pasteEffect = { _, _ in }
        let clock = TextDeliveryTimingClock()
        let delivery = TextDelivery(access: access, schedule: clock.schedule)
        delivery.target = access.target(web: true)
        var results: [Bool] = []
        delivery.commit(id: 1, text: "x") { results.append($0); _ = $1 }
        clock.advance(to: 0.349)
        XCTAssertTrue(results.isEmpty)
        clock.advance(to: 0.35)
        XCTAssertEqual(results, [false])
        access.insert("x")
        delivery.commit(id: 1, text: "x") { results.append($0); _ = $1 }
        XCTAssertEqual(results, [false, false])
        XCTAssertEqual(access.pastes, 1)
        clock.advance(to: 0.75)
        XCTAssertEqual(access.restorations, 1)
    }

    func testEarlyContextFailureCannotBecomeSuccessAfterFocusReturns() {
        let access = TextDeliveryMemoryAccess()
        let clock = TextDeliveryTimingClock()
        let delivery = TextDelivery(access: access, schedule: clock.schedule)
        delivery.target = access.target(web: true)
        var results: [Bool] = []
        delivery.commit(id: 1, text: "x") { results.append($0); _ = $1 }
        access.contextValid = false
        clock.advance(to: 0.05)
        XCTAssertEqual(results, [false])
        access.contextValid = true
        clock.advance(to: 0.35)
        XCTAssertEqual(results, [false])
        XCTAssertEqual(access.current.text, "draftx")
        delivery.commit(id: 1, text: "x") { results.append($0); _ = $1 }
        XCTAssertEqual(results, [false, false])
        XCTAssertEqual(access.pastes, 1)
        clock.advance(to: 0.75)
        XCTAssertEqual(access.restorations, 1)
    }

    func testFinalContextFailureRejectsExactText() {
        let access = TextDeliveryMemoryAccess()
        access.pasteEffect = { _, _ in }
        let clock = TextDeliveryTimingClock()
        let delivery = TextDelivery(access: access, schedule: clock.schedule)
        delivery.target = access.target(web: true)
        var result: Bool?
        delivery.commit(id: 1, text: "x") { result = $0; _ = $1 }
        clock.advance(to: 0.05)
        access.insert("x")
        access.contextValid = false
        clock.advance(to: 0.35)
        XCTAssertEqual(result, false)
        XCTAssertEqual(access.pastes, 1)
    }
}
