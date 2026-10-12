//! 网页优先粘贴及迟到回调保护；只用内存替身，不发送任何真实按键。
import XCTest
@testable import VakyuVoice

final class TextDeliveryPasteTests: XCTestCase {
    func testWebTargetSkipsWritableAxAndConfirmsOnlyAfterReadback() {
        let access = TextDeliveryMemoryAccess()
        let delivery = TextDelivery(access: access, schedule: access.schedule)
        delivery.target = access.target(web: true)
        var results: [Bool] = []
        delivery.commit(id: 1, text: "x") { results.append($0); _ = $1 }
        XCTAssertTrue(access.writes.isEmpty)
        XCTAssertEqual(access.pastes, 1)
        XCTAssertTrue(results.isEmpty)
        delivery.commit(id: 1, text: "x") { results.append($0); _ = $1 }
        XCTAssertEqual(results, [false])
        XCTAssertEqual(access.pastes, 1)
        access.runAll()
        XCTAssertEqual(results, [false, true])
        XCTAssertEqual(access.restorations, 1)
    }

    func testPasteContainsPartialOrMissingResultIsUncertainAndNeverRepeated() {
        for after in ["other x content", "draftxx", "draft", nil] {
            let access = TextDeliveryMemoryAccess()
            access.pasteEffect = { memory, _ in memory.current = SelectionSnapshot(text: after, range: nil) }
            let delivery = TextDelivery(access: access, schedule: access.schedule)
            delivery.target = access.target(web: true)
            var results: [Bool] = []
            delivery.commit(id: 1, text: "x") { results.append($0); _ = $1 }
            access.runAll()
            delivery.commit(id: 1, text: "x") { results.append($0); _ = $1 }
            XCTAssertEqual(results, [false, false])
            XCTAssertEqual(access.pastes, 1)
            XCTAssertTrue(access.writes.isEmpty)
        }
    }

    func testUnreadableBaselineCannotBeConfirmedByContains() {
        let access = TextDeliveryMemoryAccess(text: nil, range: nil)
        access.pasteEffect = { memory, _ in memory.current = SelectionSnapshot(text: "x", range: CFRange(location: 1, length: 0)) }
        let delivery = TextDelivery(access: access, schedule: access.schedule)
        delivery.target = access.target(web: true)
        var results: [Bool] = []
        delivery.commit(id: 1, text: "x") { results.append($0); _ = $1 }
        access.runAll()
        delivery.commit(id: 2, text: "x") { results.append($0); _ = $1 }
        XCTAssertEqual(results, [false, false])
        XCTAssertEqual(access.pastes, 1)
    }

    func testFocusChangeAfterPasteCannotConfirmEvenExactText() {
        let access = TextDeliveryMemoryAccess()
        let delivery = TextDelivery(access: access, schedule: access.schedule)
        delivery.target = access.target(web: true)
        var result: Bool?
        delivery.commit(id: 1, text: "x") { result = $0; _ = $1 }
        access.contextValid = false
        access.runAll()
        XCTAssertEqual(result, false)
        XCTAssertEqual(access.current.text, "draftx")
        access.contextValid = true
        delivery.commit(id: 1, text: "x") { result = $0; _ = $1 }
        XCTAssertEqual(result, false)
        XCTAssertEqual(access.pastes, 1)
    }

    func testPreparationFailureBeforeAnyInputAllowsSafeRetry() {
        let access = TextDeliveryMemoryAccess()
        access.pastePreparationFails = true
        let delivery = TextDelivery(access: access, schedule: access.schedule)
        delivery.target = access.target(web: true)
        var results: [Bool] = []
        delivery.commit(id: 1, text: "x") { results.append($0); _ = $1 }
        XCTAssertEqual(results, [false])
        XCTAssertEqual(access.pastes, 0)
        access.pastePreparationFails = false
        delivery.commit(id: 1, text: "x") { results.append($0); _ = $1 }
        access.runAll()
        XCTAssertEqual(results, [false, true])
        XCTAssertEqual(access.pastes, 1)
    }

    func testResetDiscardsOldConfirmationAndRestoresOnlyOldClipboard() {
        let access = TextDeliveryMemoryAccess()
        let delivery = TextDelivery(access: access, schedule: access.schedule)
        delivery.target = access.target(web: true)
        var oldResults: [Bool] = []
        delivery.commit(id: 1, text: "x") { oldResults.append($0); _ = $1 }
        delivery.reset()
        delivery.target = access.target(web: true)
        var newResults: [Bool] = []
        delivery.commit(id: 2, text: "y") { newResults.append($0); _ = $1 }
        access.runAll()
        XCTAssertTrue(oldResults.isEmpty)
        XCTAssertEqual(newResults, [true])
        XCTAssertEqual(access.pastes, 2)
        XCTAssertEqual(access.restorations, 2)
    }

    func testChangedDraftBeforeDeliveryNeverCallsAxOrPaste() {
        let access = TextDeliveryMemoryAccess()
        let delivery = TextDelivery(access: access, schedule: access.schedule)
        delivery.target = access.target(web: true)
        access.current = SelectionSnapshot(text: "new draft", range: CFRange(location: 9, length: 0))
        var result: Bool?
        delivery.commit(id: 1, text: "x") { result = $0; _ = $1 }
        XCTAssertEqual(result, false)
        XCTAssertTrue(access.writes.isEmpty)
        XCTAssertEqual(access.pastes, 0)
    }
}
