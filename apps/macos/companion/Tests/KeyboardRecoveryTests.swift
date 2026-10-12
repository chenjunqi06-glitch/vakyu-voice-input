//! 回放漏掉普通键松开和 Fn 事件，确保旧状态不能永久阻塞快捷键。
import XCTest
import ApplicationServices
@testable import VakyuVoice

final class KeyboardRecoveryTests: XCTestCase {
    private let option = CGEventFlags(rawValue: CGEventFlags.maskAlternate.rawValue | 0x20)

    func testStaleStartupLetterClearsBeforeFirstPressAndAppliesQueuedShortcut() throws {
        let source = KeyboardMonitorFakeSource()
        var query: Set<UInt16> = [0]
        let monitor = KeyboardMonitor(events: source, keyState: { query.contains($0) })
        XCTAssertTrue(monitor.start())
        query = []
        let saved = try Shortcut("right_win+right_alt")
        monitor.configure(enabled: true, practiceEnabled: true, dictationKey: "left_alt", shortcut: saved)
        for _ in 0..<3 {
            XCTAssertTrue(monitor.start())
            source.send(.flagsChanged, code: 58, flags: option)
            XCTAssertEqual(monitor.heldMode, .dictation)
            XCTAssertEqual(monitor.shortcut, saved)
            source.send(.flagsChanged, code: 58)
            XCTAssertNil(monitor.heldMode)
        }
        XCTAssertEqual(source.starts, 1)
    }

    func testCurrentOrdinaryEventWinsAndQueryCannotInsertOtherKeys() throws {
        let source = KeyboardMonitorFakeSource()
        var query: Set<UInt16> = []
        let monitor = KeyboardMonitor(events: source, keyState: { query.contains($0) })
        monitor.configure(enabled: true, practiceEnabled: true, dictationKey: "left_alt", shortcut: try Shortcut("f8"))
        XCTAssertTrue(monitor.start())
        query = [0]
        source.send(.keyDown, code: 100, flags: [.maskSecondaryFn])
        XCTAssertEqual(monitor.heldMode, .practice)
        source.send(.keyDown, code: 100, flags: [.maskSecondaryFn])
        XCTAssertEqual(monitor.heldMode, .practice)
        query = [0, 100]
        source.send(.keyUp, code: 100, flags: [.maskSecondaryFn])
        XCTAssertNil(monitor.heldMode)
        source.send(.flagsChanged, code: 58, flags: option)
        XCTAssertEqual(monitor.heldMode, .dictation)
    }

    func testStaleStartupFnReleaseWinsOverGlobalQuery() {
        let source = KeyboardMonitorFakeSource()
        let monitor = KeyboardMonitor(events: source, keyState: { $0 == 63 })
        XCTAssertTrue(monitor.start())
        source.send(.flagsChanged, code: 63)
        for _ in 0..<3 {
            source.send(.flagsChanged, code: 58, flags: option)
            XCTAssertEqual(monitor.heldMode, .dictation)
            source.send(.flagsChanged, code: 58)
            XCTAssertNil(monitor.heldMode)
        }
    }

    func testPhysicalFnHeldBlocksOptionUntilActualRelease() {
        let source = KeyboardMonitorFakeSource()
        let monitor = KeyboardMonitor(events: source, keyState: { _ in false })
        XCTAssertTrue(monitor.start())
        source.send(.flagsChanged, code: 63, flags: [.maskSecondaryFn])
        source.send(.flagsChanged, code: 58, flags: option.union(.maskSecondaryFn))
        XCTAssertNil(monitor.heldMode)
        source.send(.flagsChanged, code: 58, flags: [.maskSecondaryFn])
        source.send(.flagsChanged, code: 63)
        source.send(.flagsChanged, code: 58, flags: option)
        XCTAssertEqual(monitor.heldMode, .dictation)
    }

    func testFunctionAndArrowFlagsDoNotInventPhysicalFn() {
        for code: UInt16 in [100, 123, 124, 125, 126] {
            XCTAssertEqual(ModifierKeyState.updating([], flags: [.maskSecondaryFn], changed: code,
                flagsChanged: false, keyState: { _ in true }), [])
            XCTAssertEqual(ModifierKeyState.updating([63], flags: [.maskSecondaryFn], changed: code,
                flagsChanged: false, keyState: { _ in false }), [63])
        }
        let source = KeyboardMonitorFakeSource()
        let monitor = KeyboardMonitor(events: source, keyState: { _ in false })
        XCTAssertTrue(monitor.start())
        source.send(.keyDown, code: 123, flags: [.maskSecondaryFn])
        source.send(.keyUp, code: 123, flags: [.maskSecondaryFn])
        source.send(.flagsChanged, code: 58, flags: option)
        XCTAssertEqual(monitor.heldMode, .dictation)
    }

    func testMissedFunctionReleaseDispatchesBeforeNewPress() throws {
        let source = KeyboardMonitorFakeSource()
        var query: Set<UInt16> = []
        let monitor = KeyboardMonitor(events: source, keyState: { query.contains($0) })
        monitor.configure(enabled: true, practiceEnabled: true, dictationKey: "left_alt", shortcut: try Shortcut("f8"))
        var transitions: [String] = []
        monitor.onPress = { transitions.append("press:\($0 == .practice ? "practice" : "dictation")") }
        monitor.onRelease = { transitions.append("release:\($0 == .practice ? "practice" : "dictation")") }
        monitor.onInterrupted = { transitions.append("interrupted") }
        XCTAssertTrue(monitor.start())
        query = [100]
        source.send(.keyDown, code: 100, flags: [.maskSecondaryFn])
        query = []
        source.send(.flagsChanged, code: 58, flags: option)
        XCTAssertEqual(monitor.heldMode, .dictation)
        RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.01))
        XCTAssertEqual(transitions, ["press:practice", "release:practice", "press:dictation"])
    }

    func testMissedMainKeyReleaseKeepsResidualCommandBlocked() throws {
        let source = KeyboardMonitorFakeSource()
        var query: Set<UInt16> = []
        let monitor = KeyboardMonitor(events: source, keyState: { query.contains($0) })
        monitor.configure(enabled: true, practiceEnabled: true, dictationKey: "left_alt", shortcut: try Shortcut("win+f8"))
        var releases = 0
        var interruptions = 0
        monitor.onRelease = { _ in releases += 1 }
        monitor.onInterrupted = { interruptions += 1 }
        XCTAssertTrue(monitor.start())
        query = [55, 100]
        source.send(.flagsChanged, code: 55, flags: [.maskCommand])
        source.send(.keyDown, code: 100, flags: [.maskCommand, .maskSecondaryFn])
        XCTAssertEqual(monitor.heldMode, .practice)
        query = [55]
        source.send(.keyDown, code: 0, flags: [.maskCommand])
        XCTAssertNil(monitor.heldMode)
        source.send(.keyUp, code: 0, flags: [.maskCommand])
        source.send(.flagsChanged, code: 58, flags: option.union(.maskCommand))
        source.send(.flagsChanged, code: 55, flags: option)
        XCTAssertNil(monitor.heldMode)
        source.send(.flagsChanged, code: 58)
        source.send(.flagsChanged, code: 58, flags: option)
        XCTAssertEqual(monitor.heldMode, .dictation)
        RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.01))
        XCTAssertEqual(releases, 1)
        XCTAssertEqual(interruptions, 0)
    }
}
