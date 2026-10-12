//! 早期确认期间重置或重入不能追加输入，旧剪贴板恢复独立遵守时限。
import XCTest
@testable import VakyuVoice

final class TextDeliveryTimingResetTests: XCTestCase {
    func testResetBeforeEarlyCheckSuppressesOldCallbackAndKeepsRestoreDeadline() {
        let access = TextDeliveryMemoryAccess()
        let clock = TextDeliveryTimingClock()
        let delivery = TextDelivery(access: access, schedule: clock.schedule)
        delivery.target = access.target(web: true)
        var oldResults: [Bool] = []
        var newResults: [Bool] = []
        delivery.commit(id: 1, text: "x") { oldResults.append($0); _ = $1 }
        clock.advance(to: 0.02)
        delivery.reset()
        delivery.target = access.target(web: true)
        delivery.commit(id: 2, text: "y") { newResults.append($0); _ = $1 }
        clock.advance(to: 0.07)
        XCTAssertTrue(oldResults.isEmpty)
        XCTAssertEqual(newResults, [true])
        XCTAssertEqual(access.pastes, 2)
        clock.advance(to: 0.749)
        XCTAssertEqual(access.restorations, 0)
        clock.advance(to: 0.75)
        XCTAssertEqual(access.restorations, 1)
        clock.advance(to: 0.77)
        XCTAssertEqual(access.restorations, 2)
        XCTAssertTrue(oldResults.isEmpty)
        XCTAssertEqual(newResults, [true])
    }

    func testResetDuringReadbackCannotConfirmStaleOperation() {
        let access = TextDeliveryMemoryAccess()
        let clock = TextDeliveryTimingClock()
        let delivery = TextDelivery(access: access, schedule: clock.schedule)
        delivery.target = access.target(web: true)
        var results: [Bool] = []
        delivery.commit(id: 1, text: "x") { results.append($0); _ = $1 }
        access.snapshotEffect = { _ in delivery.reset() }
        clock.advance(to: 0.05)
        XCTAssertTrue(results.isEmpty)
        clock.advance(to: 0.749)
        XCTAssertTrue(results.isEmpty)
        XCTAssertEqual(access.restorations, 0)
        clock.advance(to: 0.75)
        XCTAssertEqual(access.restorations, 1)
        XCTAssertEqual(access.pastes, 1)
    }

    func testEarlyCompletionReentryCannotPasteTwice() {
        let access = TextDeliveryMemoryAccess()
        let clock = TextDeliveryTimingClock()
        let delivery = TextDelivery(access: access, schedule: clock.schedule)
        delivery.target = access.target(web: true)
        var results: [Bool] = []
        delivery.commit(id: 1, text: "x") { ok, _ in
            results.append(ok)
            delivery.commit(id: 1, text: "x") { results.append($0); _ = $1 }
        }
        clock.advance(to: 0.05)
        XCTAssertEqual(results, [true, true])
        clock.advance(to: 0.75)
        XCTAssertEqual(results, [true, true])
        XCTAssertEqual(access.pastes, 1)
        XCTAssertEqual(access.restorations, 1)
    }
}
