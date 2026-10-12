//! 按真实修饰键事件顺序回放录入，验证左右键与 Option 松开不会误保存。
import XCTest
@testable import VakyuVoice

final class ShortcutRecorderTests: XCTestCase {
    func testRecordsRightCommandAndControlChordsInBothOrders() throws {
        for modifier in [UInt16(54), 62] {
            for first in [modifier, 61] {
                for remaining in [modifier, 61] {
                    var recorder = ShortcutRecorder()
                    XCTAssertNil(try recorder.update(down: [first], changed: first, isDown: true, flags: []))
                    XCTAssertNil(try recorder.update(down: [modifier, 61], changed: first == 61 ? modifier : 61,
                                                     isDown: true, flags: [.maskAlternate]))
                    XCTAssertNil(try recorder.update(down: [remaining], changed: remaining == 61 ? modifier : 61,
                                                     isDown: false, flags: []))
                    let shortcut = try recorder.update(down: [], changed: remaining, isDown: false, flags: [])
                    XCTAssertEqual(shortcut?.canonical, modifier == 54 ? "right_win+right_alt" : "right_ctrl+right_alt")
                }
            }
        }
    }

    func testStandaloneOptionOnlyCompletesOnRelease() throws {
        for code in [UInt16(58), 61] {
            var recorder = ShortcutRecorder()
            XCTAssertNil(try recorder.update(down: [code], changed: code, isDown: true, flags: [.maskAlternate]))
            let shortcut = try recorder.update(down: [], changed: code, isDown: false, flags: [])
            XCTAssertEqual(shortcut?.canonical, code == 58 ? "left_alt" : "right_alt")
        }
    }

    func testUnsupportedModifierChordCannotBecomeStandaloneOption() throws {
        for remaining in [UInt16(55), 61] {
            var recorder = ShortcutRecorder()
            XCTAssertNil(try recorder.update(down: [61], changed: 61, isDown: true, flags: [.maskAlternate]))
            XCTAssertNil(try recorder.update(down: [55, 61], changed: 55, isDown: true, flags: [.maskCommand, .maskAlternate]))
            XCTAssertNil(try recorder.update(down: [remaining], changed: remaining == 55 ? 61 : 55, isDown: false, flags: []))
            XCTAssertThrowsError(try recorder.update(down: [], changed: remaining, isDown: false, flags: []))
        }
    }

    func testRecordsCharacterCombinationAndFunctionKey() throws {
        var recorder = ShortcutRecorder()
        XCTAssertNil(try recorder.update(down: [59], changed: 59, isDown: true, flags: [.maskControl]))
        XCTAssertNil(try recorder.update(down: [59, 58], changed: 58, isDown: true, flags: [.maskControl, .maskAlternate]))
        let shortcut = try recorder.update(down: [59, 58, 14], changed: 14, isDown: true,
                                           flags: [.maskControl, .maskAlternate])
        XCTAssertEqual(shortcut?.canonical, "ctrl+alt+e")
        recorder = ShortcutRecorder()
        XCTAssertEqual(try recorder.update(down: [100], changed: 100, isDown: true, flags: [])?.canonical, "f8")
    }

    func testModifierChordCanContinueIntoCharacterCombination() throws {
        var recorder = ShortcutRecorder()
        XCTAssertNil(try recorder.update(down: [54], changed: 54, isDown: true, flags: [.maskCommand]))
        XCTAssertNil(try recorder.update(down: [54, 61], changed: 61, isDown: true, flags: [.maskCommand, .maskAlternate]))
        let shortcut = try recorder.update(down: [54, 61, 14], changed: 14, isDown: true,
                                           flags: [.maskCommand, .maskAlternate])
        XCTAssertEqual(shortcut?.canonical, "alt+win+e")
    }

    func testExtraModifierInvalidatesPendingChord() throws {
        var recorder = ShortcutRecorder()
        XCTAssertNil(try recorder.update(down: [54], changed: 54, isDown: true, flags: []))
        XCTAssertNil(try recorder.update(down: [54, 61], changed: 61, isDown: true, flags: []))
        XCTAssertNil(try recorder.update(down: [54, 61, 56], changed: 56, isDown: true, flags: []))
        XCTAssertNil(try recorder.update(down: [54, 61], changed: 56, isDown: false, flags: []))
        XCTAssertNil(try recorder.update(down: [61], changed: 54, isDown: false, flags: []))
        XCTAssertThrowsError(try recorder.update(down: [], changed: 61, isDown: false, flags: []))
    }
}
