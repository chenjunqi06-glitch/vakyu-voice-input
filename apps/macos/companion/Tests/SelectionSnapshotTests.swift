// 保护原草稿与 UTF-16 选区的测试，不触碰系统剪贴板或其他应用。
import XCTest
@testable import VakyuVoice

final class SelectionSnapshotTests: XCTestCase {
    func testChangedDraftAndCursorAreRejected() {
        let original = SelectionSnapshot(text: "hello", range: CFRange(location: 5, length: 0))
        XCTAssertTrue(original.unchanged(text: "hello", range: CFRange(location: 5, length: 0)))
        XCTAssertFalse(original.unchanged(text: "new draft", range: CFRange(location: 5, length: 0)))
        XCTAssertFalse(original.unchanged(text: "hello", range: CFRange(location: 2, length: 0)))
        XCTAssertFalse(original.unchanged(text: "hello", range: nil))
        XCTAssertFalse(original.unchanged(text: nil, range: nil))
    }

    func testChineseAndEmojiUseUtf16Positions() {
        let snapshot = SelectionSnapshot(text: "你好😀 world", range: CFRange(location: 2, length: 2))
        XCTAssertEqual(snapshot.replacing(with: "世界"), "你好世界 world")
        let cursor = SelectionSnapshot(text: "😀", range: CFRange(location: 2, length: 0))
        XCTAssertEqual(cursor.replacing(with: "你好"), "😀你好")
    }

    func testInvalidRangesNeverReplaceEntireDraft() {
        for range in [CFRange(location: -1, length: 0), CFRange(location: 4, length: 0), CFRange(location: 1, length: 9)] {
            XCTAssertNil(SelectionSnapshot(text: "abc", range: range).replacing(with: "x"))
        }
    }
}
