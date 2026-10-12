//! 松键事件快照优先于全局查询，避免正在聆听状态和录音无法结束。
import XCTest
import ApplicationServices
@testable import VakyuVoice

final class KeyboardReleaseTests: XCTestCase {
    func testOptionReleaseWinsOverStaleGlobalState() {
        let source = KeyboardMonitorFakeSource()
        var query: Set<UInt16> = []
        let monitor = KeyboardMonitor(events: source, keyState: { query.contains($0) })
        XCTAssertTrue(monitor.start())
        query = [58]
        source.send(.flagsChanged, code: 58, flags: CGEventFlags(rawValue: CGEventFlags.maskAlternate.rawValue | 0x20))
        XCTAssertEqual(monitor.heldMode, .dictation)
        let released = expectation(description: "明确松键会结束本轮")
        released.assertForOverFulfill = true
        monitor.onRelease = { mode in XCTAssertEqual(mode, .dictation); released.fulfill() }
        source.send(.flagsChanged, code: 58, flags: [])
        XCTAssertNil(monitor.heldMode)
        source.send(.flagsChanged, code: 58, flags: [])
        wait(for: [released], timeout: 1)
    }

    func testEventSnapshotStartsEvenWhenGlobalStateAlreadyReleased() {
        let source = KeyboardMonitorFakeSource()
        let monitor = KeyboardMonitor(events: source, keyState: { _ in false })
        XCTAssertTrue(monitor.start())
        source.send(.flagsChanged, code: 58, flags: CGEventFlags(rawValue: CGEventFlags.maskAlternate.rawValue | 0x20))
        XCTAssertEqual(monitor.heldMode, .dictation)
        source.send(.flagsChanged, code: 58)
        XCTAssertNil(monitor.heldMode)
    }

    func testRightChordReleasesInEitherOrderWithStaleGlobalState() throws {
        for firstRelease in [UInt16(54), 61] {
            let source = KeyboardMonitorFakeSource()
            var query: Set<UInt16> = []
            let monitor = KeyboardMonitor(events: source, keyState: { query.contains($0) })
            monitor.configure(enabled: true, practiceEnabled: true, dictationKey: "left_alt",
                              shortcut: try Shortcut("right_win+right_alt"))
            XCTAssertTrue(monitor.start())
            query = [54, 61]
            source.send(.flagsChanged, code: 54, flags: CGEventFlags(rawValue: CGEventFlags.maskCommand.rawValue | 0x10))
            source.send(.flagsChanged, code: 61, flags: CGEventFlags(rawValue: CGEventFlags.maskCommand.rawValue | CGEventFlags.maskAlternate.rawValue | 0x10 | 0x40))
            XCTAssertEqual(monitor.heldMode, .practice)
            let remainingFlags = firstRelease == 54
                ? CGEventFlags(rawValue: CGEventFlags.maskAlternate.rawValue | 0x40)
                : CGEventFlags(rawValue: CGEventFlags.maskCommand.rawValue | 0x10)
            source.send(.flagsChanged, code: firstRelease, flags: remainingFlags)
            XCTAssertNil(monitor.heldMode)
        }
    }

    func testOtherOptionStillHeldCannotHideReleaseOfDictationSide() {
        let previous: Set<UInt16> = [58, 61]
        let next = ModifierKeyState.updating(previous,
            flags: CGEventFlags(rawValue: CGEventFlags.maskAlternate.rawValue | 0x40),
            changed: 58, flagsChanged: true, keyState: { _ in true })
        XCTAssertEqual(next, [61])
    }

    func testGenericFlagsFollowChangedSideWhenDeviceBitsAreMissing() {
        var down: Set<UInt16> = []
        down = ModifierKeyState.updating(down, flags: [.maskAlternate], changed: 58, flagsChanged: true, keyState: { _ in false })
        XCTAssertEqual(down, [58])
        down = ModifierKeyState.updating(down, flags: [.maskAlternate], changed: 61, flagsChanged: true, keyState: { _ in false })
        XCTAssertEqual(down, [58, 61])
        down = ModifierKeyState.updating(down, flags: [.maskAlternate], changed: 58, flagsChanged: true, keyState: { _ in true })
        XCTAssertEqual(down, [61])
        down = ModifierKeyState.updating(down, flags: [], changed: 61, flagsChanged: true, keyState: { _ in true })
        XCTAssertTrue(down.isEmpty)
    }

    func testMainKeyReleaseIsNotReinsertedByGlobalQuery() throws {
        let source = KeyboardMonitorFakeSource()
        var query: Set<UInt16> = []
        let monitor = KeyboardMonitor(events: source, keyState: { query.contains($0) })
        monitor.configure(enabled: true, practiceEnabled: true, dictationKey: "left_alt", shortcut: try Shortcut("win+v"))
        XCTAssertTrue(monitor.start())
        query = [55, 9]
        source.send(.flagsChanged, code: 55, flags: [.maskCommand])
        source.send(.keyDown, code: 9, flags: [.maskCommand])
        XCTAssertEqual(monitor.heldMode, .practice)
        source.send(.keyUp, code: 9, flags: [.maskCommand])
        XCTAssertNil(monitor.heldMode)
    }
}
