//! 用内存 AX 替身验证真实 TextDelivery；API 返回成功但未写入不能误报。
import XCTest
import ApplicationServices
@testable import VakyuVoice

final class TextDeliveryNativeTests: XCTestCase {
    func testNativeNoChangeFallsBackOnceAndConfirmsExactPaste() {
        let access = TextDeliveryMemoryAccess()
        access.writeEffect = { _, _, _ in }
        let delivery = TextDelivery(access: access, schedule: access.schedule)
        delivery.target = access.target()
        var results: [Bool] = []
        delivery.commit(id: 1, text: "x") { results.append($0); _ = $1 }
        XCTAssertTrue(results.isEmpty)
        XCTAssertEqual(access.writes, [kAXSelectedTextAttribute])
        XCTAssertEqual(access.pastes, 0)
        access.runNext()
        XCTAssertEqual(access.pastes, 1)
        XCTAssertTrue(results.isEmpty)
        access.runAll()
        XCTAssertEqual(results, [true])
        XCTAssertEqual(access.current.text, "draftx")
        XCTAssertEqual(access.restorations, 1)
        delivery.commit(id: 1, text: "x") { results.append($0); _ = $1 }
        XCTAssertEqual(results, [true, true])
        XCTAssertEqual(access.writes.count, 1)
        XCTAssertEqual(access.pastes, 1)
    }

    func testNativeSelectedTextNeedsReadbackAndValuePathKeepsUtf16Caret() {
        for selectedWritable in [true, false] {
            let access = TextDeliveryMemoryAccess(text: "前😀后", range: CFRange(location: 1, length: 2))
            access.selectedWritable = selectedWritable
            let delivery = TextDelivery(access: access, schedule: access.schedule)
            delivery.target = access.target()
            var result: Bool?
            delivery.commit(id: 1, text: "中文") { result = $0; _ = $1 }
            XCTAssertEqual(result, true)
            XCTAssertEqual(access.current.text, "前中文后")
            XCTAssertEqual(access.writes, [selectedWritable ? kAXSelectedTextAttribute : kAXValueAttribute])
            XCTAssertEqual(access.pastes, 0)
            XCTAssertEqual(access.caretPositions, selectedWritable ? [] : [3])
        }
    }

    func testPartialMissingOrSelectionOnlyChangeNeverFallsBackOrRepeats() {
        for snapshot in [SelectionSnapshot(text: "partial", range: CFRange(location: 7, length: 0)),
                         SelectionSnapshot(text: nil, range: nil),
                         SelectionSnapshot(text: "draft", range: CFRange(location: 0, length: 0))] {
            let access = TextDeliveryMemoryAccess()
            access.writeEffect = { memory, _, _ in memory.current = snapshot }
            let delivery = TextDelivery(access: access, schedule: access.schedule)
            delivery.target = access.target()
            var results: [Bool] = []
            delivery.commit(id: 1, text: "x") { results.append($0); _ = $1 }
            delivery.commit(id: 1, text: "x") { results.append($0); _ = $1 }
            delivery.commit(id: 2, text: "y") { results.append($0); _ = $1 }
            access.runAll()
            XCTAssertEqual(results, [false, false, false])
            XCTAssertEqual(access.writes.count, 1)
            XCTAssertEqual(access.pastes, 0)
        }
    }

    func testDelayedNativeApplicationIsConfirmedWithoutPaste() {
        let access = TextDeliveryMemoryAccess()
        access.writeEffect = { _, _, _ in }
        let delivery = TextDelivery(access: access, schedule: access.schedule)
        delivery.target = access.target()
        var result: Bool?
        delivery.commit(id: 1, text: "x") { result = $0; _ = $1 }
        XCTAssertNil(result)
        access.insert("x")
        access.runAll()
        XCTAssertEqual(result, true)
        XCTAssertEqual(access.writes.count, 1)
        XCTAssertEqual(access.pastes, 0)
    }

    func testDraftChangeBeforeFallbackLocksFurtherAttempts() {
        let access = TextDeliveryMemoryAccess()
        access.writeEffect = { _, _, _ in }
        let delivery = TextDelivery(access: access, schedule: access.schedule)
        delivery.target = access.target()
        var results: [Bool] = []
        delivery.commit(id: 1, text: "x") { results.append($0); _ = $1 }
        access.validateEffect = { memory, protectDraft in
            if protectDraft { memory.current = SelectionSnapshot(text: "edited", range: CFRange(location: 6, length: 0)) }
        }
        access.runAll()
        access.validateEffect = nil
        access.current = SelectionSnapshot(text: "draft", range: CFRange(location: 5, length: 0))
        delivery.commit(id: 1, text: "x") { results.append($0); _ = $1 }
        XCTAssertEqual(results, [false, false])
        XCTAssertEqual(access.writes.count, 1)
        XCTAssertEqual(access.pastes, 0)
    }

    func testResetCancelsPendingNativeFallback() {
        let access = TextDeliveryMemoryAccess()
        access.writeEffect = { _, _, _ in }
        let delivery = TextDelivery(access: access, schedule: access.schedule)
        delivery.target = access.target()
        var results: [Bool] = []
        delivery.commit(id: 1, text: "x") { results.append($0); _ = $1 }
        delivery.reset()
        access.runAll()
        XCTAssertTrue(results.isEmpty)
        XCTAssertEqual(access.pastes, 0)
        access.writeEffect = nil
        delivery.target = access.target()
        delivery.commit(id: 2, text: "y") { results.append($0); _ = $1 }
        XCTAssertEqual(results, [true])
        XCTAssertEqual(access.current.text, "drafty")
    }

    func testWriteAndCompletionReentryCannotInjectTwice() {
        let access = TextDeliveryMemoryAccess()
        let delivery = TextDelivery(access: access, schedule: access.schedule)
        delivery.target = access.target()
        var busy: Bool?
        var repeated: Bool?
        access.writeEffect = { memory, _, text in
            delivery.commit(id: 2, text: "other") { busy = $0; _ = $1 }
            memory.insert(text)
        }
        delivery.commit(id: 1, text: "x") { result, _ in
            XCTAssertTrue(result)
            delivery.commit(id: 1, text: "x") { repeated = $0; _ = $1 }
        }
        XCTAssertEqual(busy, false)
        XCTAssertEqual(repeated, true)
        XCTAssertEqual(access.writes.count, 1)
        XCTAssertEqual(access.current.text, "draftx")
    }

    func testAlreadyEqualReplacementConfirmsWithoutSendingAnyInput() {
        let access = TextDeliveryMemoryAccess(text: "same", range: CFRange(location: 0, length: 4))
        let delivery = TextDelivery(access: access, schedule: access.schedule)
        delivery.target = access.target()
        var result: Bool?
        delivery.commit(id: 1, text: "same") { result = $0; _ = $1 }
        XCTAssertEqual(result, true)
        XCTAssertTrue(access.writes.isEmpty)
        XCTAssertEqual(access.pastes, 0)
    }

    func testKnownInvalidSelectionNeverSendsNativeOrWebInput() {
        for web in [false, true] {
            let access = TextDeliveryMemoryAccess(text: "😀x", range: CFRange(location: 1, length: 0))
            let delivery = TextDelivery(access: access, schedule: access.schedule)
            delivery.target = access.target(web: web)
            var result: Bool?
            delivery.commit(id: 1, text: "x") { result = $0; _ = $1 }
            XCTAssertEqual(result, false)
            XCTAssertTrue(access.writes.isEmpty)
            XCTAssertEqual(access.pastes, 0)
        }
    }
}
