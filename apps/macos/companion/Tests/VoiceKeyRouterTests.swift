// 两个入口的纯状态机回归，不安装事件钩子，也不需要按真实键。
import XCTest
@testable import VakyuVoice

final class VoiceKeyRouterTests: XCTestCase {
    func testLeftOptionAlwaysSelectsDictationEvenWhenPracticeEnabled() {
        var router = VoiceKeyRouter()
        router.practiceEnabled = true
        XCTAssertEqual(router.update(down: [58], changed: 58, isDown: true), .pressed(.dictation))
        XCTAssertEqual(router.update(down: [], changed: 58, isDown: false), .released(.dictation))
    }

    func testRightChordSelectsPracticeInEitherPressOrder() {
        for first in [UInt16(61), 62] {
            var router = VoiceKeyRouter()
            router.practiceEnabled = true
            XCTAssertEqual(router.update(down: [first], changed: first, isDown: true), .none)
            XCTAssertEqual(router.update(down: [61, 62], changed: first == 61 ? 62 : 61, isDown: true), .pressed(.practice))
        }
    }

    func testRightCommandOptionSupportsBothPressAndReleaseOrders() throws {
        for first in [UInt16(54), 61] {
            for remaining in [UInt16(54), 61] {
                var router = VoiceKeyRouter()
                router.practiceEnabled = true
                router.practiceShortcut = try Shortcut("right_win+right_alt")
                XCTAssertEqual(router.update(down: [first], changed: first, isDown: true), .none)
                XCTAssertEqual(router.update(down: [54, 61], changed: first == 54 ? 61 : 54, isDown: true), .pressed(.practice))
                XCTAssertEqual(router.update(down: [remaining], changed: remaining == 54 ? 61 : 54, isDown: false), .released(.practice))
                XCTAssertEqual(router.update(down: [], changed: remaining, isDown: false), .none)
                XCTAssertEqual(router.update(down: [58], changed: 58, isDown: true), .pressed(.dictation))
            }
        }
    }

    func testRightCommandChordRejectsOtherSidesAndExtraModifiers() throws {
        for down: Set<UInt16> in [[55, 61], [54, 58], [54, 61, 62], [54, 55, 61]] {
            var router = VoiceKeyRouter()
            router.practiceEnabled = true
            router.practiceShortcut = try Shortcut("right_win+right_alt")
            XCTAssertEqual(router.update(down: down, changed: 61, isDown: true), .none)
        }
    }

    func testRightCommandChordReservesLegacyRightOptionPrefixAndDetectsConflict() throws {
        let shortcut = try Shortcut("right_win+right_alt")
        var router = VoiceKeyRouter()
        router.practiceEnabled = true
        router.dictationKey = "alt"
        router.practiceShortcut = shortcut
        XCTAssertEqual(router.update(down: [61], changed: 61, isDown: true), .none)
        XCTAssertEqual(router.update(down: [54, 61], changed: 54, isDown: true), .pressed(.practice))
        XCTAssertEqual(router.update(down: [61], changed: 54, isDown: false), .released(.practice))
        XCTAssertEqual(router.update(down: [61], changed: 61, isDown: true), .none)
        XCTAssertTrue(VoiceKeyRouter.conflicts(dictationKey: "right_alt", practice: shortcut))
        XCTAssertFalse(VoiceKeyRouter.conflicts(dictationKey: "left_alt", practice: shortcut))
        XCTAssertFalse(VoiceKeyRouter.conflicts(dictationKey: "alt", practice: shortcut))
    }

    func testDisabledPracticeDoesNotFallBackToDictation() {
        var router = VoiceKeyRouter()
        XCTAssertEqual(router.update(down: [62], changed: 62, isDown: true), .none)
        XCTAssertEqual(router.update(down: [61, 62], changed: 61, isDown: true), .none)
        XCTAssertEqual(router.update(down: [61], changed: 62, isDown: false), .none)
        XCTAssertEqual(router.update(down: [], changed: 61, isDown: false), .none)
        XCTAssertEqual(router.update(down: [58], changed: 58, isDown: true), .pressed(.dictation))
    }

    func testOptionLetterInterruptsAndResidualOptionCannotRestart() {
        var router = VoiceKeyRouter()
        XCTAssertEqual(router.update(down: [58], changed: 58, isDown: true), .pressed(.dictation))
        XCTAssertEqual(router.update(down: [58, 0], changed: 0, isDown: true), .interrupted(.dictation))
        XCTAssertNil(router.active)
        XCTAssertEqual(router.update(down: [58], changed: 0, isDown: false), .none)
        XCTAssertEqual(router.update(down: [], changed: 58, isDown: false), .none)
        XCTAssertEqual(router.update(down: [58], changed: 58, isDown: true), .pressed(.dictation))
    }

    func testMixedGesturesRequireAllKeysReleasedBeforeSwitching() {
        var router = VoiceKeyRouter()
        router.practiceEnabled = true
        XCTAssertEqual(router.update(down: [58], changed: 58, isDown: true), .pressed(.dictation))
        XCTAssertEqual(router.update(down: [58, 62], changed: 62, isDown: true), .interrupted(.dictation))
        XCTAssertEqual(router.update(down: [58, 62, 61], changed: 61, isDown: true), .none)
        XCTAssertEqual(router.update(down: [62, 61], changed: 58, isDown: false), .none)
        XCTAssertEqual(router.update(down: [], changed: 61, isDown: false), .none)
        XCTAssertEqual(router.update(down: [62, 61], changed: 61, isDown: true), .pressed(.practice))
    }

    func testEitherReleaseOrderEndsPracticeOnce() {
        for remaining in [UInt16(61), 62] {
            var router = VoiceKeyRouter()
            router.practiceEnabled = true
            XCTAssertEqual(router.update(down: [61, 62], changed: 61, isDown: true), .pressed(.practice))
            XCTAssertEqual(router.update(down: [remaining], changed: remaining == 61 ? 62 : 61, isDown: false), .released(.practice))
            XCTAssertEqual(router.update(down: [], changed: remaining, isDown: false), .none)
        }
    }

    func testLegacyBothOptionsCannotRestartFromRemainingModifier() {
        var router = VoiceKeyRouter()
        router.dictationKey = "alt"
        router.practiceEnabled = true
        XCTAssertEqual(router.update(down: [62], changed: 62, isDown: true), .none)
        XCTAssertEqual(router.update(down: [61, 62], changed: 61, isDown: true), .pressed(.practice))
        XCTAssertEqual(router.update(down: [61], changed: 62, isDown: false), .released(.practice))
        XCTAssertEqual(router.update(down: [61], changed: 61, isDown: true), .none)
        XCTAssertEqual(router.update(down: [], changed: 61, isDown: false), .none)
        XCTAssertEqual(router.update(down: [61], changed: 61, isDown: true), .none)
        XCTAssertEqual(router.update(down: [61, 62], changed: 62, isDown: true), .pressed(.practice))
    }

    func testLegacyBothOptionsLeavesRightOptionPrefixForPractice() {
        var router = VoiceKeyRouter()
        router.dictationKey = "alt"
        router.practiceEnabled = true
        XCTAssertEqual(router.update(down: [61], changed: 61, isDown: true), .none)
        XCTAssertEqual(router.update(down: [61, 62], changed: 62, isDown: true), .pressed(.practice))
        XCTAssertEqual(router.update(down: [], changed: 61, isDown: false), .released(.practice))
        XCTAssertEqual(router.update(down: [58], changed: 58, isDown: true), .pressed(.dictation))
    }

    func testCustomFunctionKeyAndMasterDisable() throws {
        var router = VoiceKeyRouter()
        router.practiceEnabled = true
        router.practiceShortcut = try Shortcut("f8")
        XCTAssertEqual(router.update(down: [100], changed: 100, isDown: true), .pressed(.practice))
        XCTAssertEqual(router.update(down: [], changed: 100, isDown: false), .released(.practice))
        router.enabled = false
        XCTAssertEqual(router.update(down: [58], changed: 58, isDown: true), .none)
        XCTAssertEqual(router.update(down: [100], changed: 100, isDown: true), .none)
    }

    func testPracticeCannotUseSamePhysicalKeyAsDictation() throws {
        XCTAssertTrue(VoiceKeyRouter.conflicts(dictationKey: "left_alt", practice: try Shortcut("left_alt")))
        XCTAssertTrue(VoiceKeyRouter.conflicts(dictationKey: "alt", practice: try Shortcut("right_alt")))
        XCTAssertTrue(VoiceKeyRouter.conflicts(dictationKey: "right_alt", practice: .defaultValue))
        XCTAssertFalse(VoiceKeyRouter.conflicts(dictationKey: "left_alt", practice: .defaultValue))
        XCTAssertFalse(VoiceKeyRouter.conflicts(dictationKey: "left_alt", practice: try Shortcut("f8")))
    }
}
