//! 精确替换、不可验证状态和网页祖先检测的纯回归。
import XCTest
@testable import VakyuVoice

final class DeliveryVerificationTests: XCTestCase {
    func testExactUnicodeReplacementConfirmsAndContainsDoesNot() {
        let before = SelectionSnapshot(text: "前😀后", range: CFRange(location: 1, length: 2))
        XCTAssertEqual(before.replacing(with: "中文"), "前中文后")
        XCTAssertEqual(DeliveryVerification.inspect(before: before, insertion: "中文",
            after: SelectionSnapshot(text: "前中文后", range: nil)), .confirmed)
        XCTAssertEqual(DeliveryVerification.inspect(before: before, insertion: "中文",
            after: SelectionSnapshot(text: "其他中文结果", range: nil)), .uncertain)
        XCTAssertEqual(DeliveryVerification.inspect(before: before, insertion: "中文",
            after: SelectionSnapshot(text: "前中文后中文", range: nil)), .uncertain)
    }

    func testUnchangedRequiresCompleteMatchingOriginalSelection() {
        let before = SelectionSnapshot(text: "draft", range: CFRange(location: 5, length: 0))
        XCTAssertEqual(DeliveryVerification.inspect(before: before, insertion: "x", after: before), .unchanged)
        for range in [nil, CFRange(location: 0, length: 0), CFRange(location: 4, length: 1)] {
            XCTAssertEqual(DeliveryVerification.inspect(before: before, insertion: "x",
                after: SelectionSnapshot(text: "draft", range: range)), .uncertain)
        }
        XCTAssertEqual(DeliveryVerification.inspect(before: before, insertion: "x",
            after: SelectionSnapshot(text: nil, range: before.range)), .uncertain)
    }

    func testMissingBaselineCannotConfirmEvenWhenResultContainsInsertion() {
        for before in [SelectionSnapshot(text: nil, range: nil),
                       SelectionSnapshot(text: "draft", range: nil),
                       SelectionSnapshot(text: nil, range: CFRange(location: 0, length: 0))] {
            XCTAssertEqual(DeliveryVerification.inspect(before: before, insertion: "x",
                after: SelectionSnapshot(text: "draftx", range: CFRange(location: 6, length: 0))), .uncertain)
        }
    }

    func testInvalidUtf16RangesNeverProduceExpectedDraft() {
        for range in [CFRange(location: -1, length: 0), CFRange(location: 0, length: -1),
                      CFRange(location: 4, length: 0), CFRange(location: 1, length: Int.max),
                      CFRange(location: Int.max, length: 1), CFRange(location: 1, length: 0)] {
            XCTAssertNil(SelectionSnapshot(text: "😀x", range: range).replacing(with: "text"))
        }
        XCTAssertEqual(SelectionSnapshot(text: "e\u{301}x", range: CFRange(location: 1, length: 1)).replacing(with: "!"), "e!x")
    }

    func testWebAreaAncestryIsBoundedAndCycleSafe() {
        let same: (Int, Int) -> Bool = { $0 == $1 }
        let role: (Int) -> String? = { $0 == 2 ? "AXWebArea" : "AXGroup" }
        let parent: (Int) -> Int? = { $0 < 3 ? $0 + 1 : nil }
        XCTAssertTrue(WebTargetPolicy.hasWebArea(from: 0, role: role, parent: parent, same: same))
        XCTAssertFalse(WebTargetPolicy.hasWebArea(from: 0, limit: 2,
            role: role, parent: { $0 + 1 }, same: same))
        var visits = 0
        XCTAssertFalse(WebTargetPolicy.hasWebArea(from: 0, role: { _ in visits += 1; return "AXGroup" },
            parent: { ($0 + 1) % 2 }, same: same))
        XCTAssertEqual(visits, 2)
        XCTAssertFalse(WebTargetPolicy.hasWebArea(from: 0, role: { _ in "AXTextArea" }, parent: { _ in nil }, same: same))
        XCTAssertTrue(WebTargetPolicy.hasWebArea(from: 0, role: { _ in "AXWebArea" }, parent: { _ in nil }, same: same))
    }
}
