//! 回放权限恢复、系统停用与丢失松键；不读取或监听真实键盘。
import XCTest
import ApplicationServices
@testable import VakyuVoice

final class KeyboardMonitorTests: XCTestCase {
    func testHealthyStartAndRepeatedStopAreIdempotent() {
        let source = KeyboardMonitorFakeSource()
        let monitor = KeyboardMonitor(events: source, keyState: { _ in false })
        var states: [Bool] = []
        monitor.onAvailabilityChanged = { states.append($0) }
        XCTAssertTrue(monitor.start())
        XCTAssertTrue(monitor.start())
        XCTAssertEqual(source.starts, 1)
        monitor.stop()
        monitor.stop()
        XCTAssertEqual(states, [true, false])
    }

    func testDisabledSourceIsRecreatedAndFailedReconnectStaysUnavailable() {
        let source = KeyboardMonitorFakeSource()
        let monitor = KeyboardMonitor(events: source, keyState: { _ in false })
        XCTAssertTrue(monitor.start())
        source.available = false
        XCTAssertFalse(monitor.available)
        XCTAssertTrue(monitor.start())
        XCTAssertEqual(source.starts, 2)
        source.available = false
        source.mayStart = false
        XCTAssertFalse(monitor.start())
        XCTAssertFalse(monitor.available)
        XCTAssertEqual(source.starts, 3)
    }

    func testFirstDictationPressWorksAfterPermissionRecovery() {
        let source = KeyboardMonitorFakeSource()
        var down: Set<UInt16> = []
        let monitor = KeyboardMonitor(events: source, keyState: { down.contains($0) })
        monitor.stop()
        XCTAssertTrue(monitor.start())
        down = [58]
        source.send(.flagsChanged, code: 58, flags: CGEventFlags(rawValue: CGEventFlags.maskAlternate.rawValue | 0x20))
        XCTAssertEqual(monitor.heldMode, .dictation)
    }

    func testReconnectDoesNotActivateModifierAlreadyHeld() {
        let source = KeyboardMonitorFakeSource()
        var down: Set<UInt16> = [58]
        let monitor = KeyboardMonitor(events: source, keyState: { down.contains($0) })
        XCTAssertTrue(monitor.start())
        source.send(.flagsChanged, code: 58, flags: CGEventFlags(rawValue: CGEventFlags.maskAlternate.rawValue | 0x20))
        XCTAssertNil(monitor.heldMode)
        down = []
        source.send(.flagsChanged, code: 58)
        down = [58]
        source.send(.flagsChanged, code: 58, flags: CGEventFlags(rawValue: CGEventFlags.maskAlternate.rawValue | 0x20))
        XCTAssertEqual(monitor.heldMode, .dictation)
    }

    func testMissedReleaseDoesNotKeepConfigurationQueued() throws {
        let source = KeyboardMonitorFakeSource()
        var down: Set<UInt16> = []
        let monitor = KeyboardMonitor(events: source, keyState: { down.contains($0) })
        XCTAssertTrue(monitor.start())
        down = [58]
        source.send(.flagsChanged, code: 58, flags: CGEventFlags(rawValue: CGEventFlags.maskAlternate.rawValue | 0x20))
        let saved = try Shortcut("win+v")
        monitor.configure(enabled: true, practiceEnabled: true, dictationKey: "left_alt", shortcut: saved)
        source.available = false
        down = []
        XCTAssertTrue(monitor.start())
        XCTAssertEqual(monitor.shortcut, saved)
        down = [55]
        source.send(.flagsChanged, code: 55, flags: [.maskCommand])
        down = [55, 9]
        source.send(.keyDown, code: 9, flags: [.maskCommand])
        XCTAssertEqual(monitor.heldMode, .practice)
    }

    func testStopCancelsRecordingOnce() {
        let source = KeyboardMonitorFakeSource()
        let monitor = KeyboardMonitor(events: source, keyState: { _ in false })
        var failures = 0
        monitor.onRecorded = { if case .failure = $0 { failures += 1 } }
        XCTAssertTrue(monitor.start())
        monitor.beginRecording()
        monitor.stop()
        monitor.stop()
        XCTAssertFalse(monitor.recording)
        XCTAssertEqual(failures, 1)
    }

    func testTimeoutReconnectsOffCallbackAndHonorsExplicitStop() {
        let source = KeyboardMonitorFakeSource()
        let monitor = KeyboardMonitor(events: source, keyState: { _ in false })
        XCTAssertTrue(monitor.start())
        source.available = false
        source.send(.tapDisabledByTimeout)
        XCTAssertEqual(source.starts, 1)
        let reconnected = expectation(description: "回调外重连")
        DispatchQueue.main.async {
            XCTAssertEqual(source.starts, 2)
            XCTAssertTrue(monitor.available)
            source.available = false
            source.send(.tapDisabledByUserInput)
            monitor.stop()
            DispatchQueue.main.async {
                XCTAssertEqual(source.starts, 2)
                XCTAssertFalse(monitor.available)
                reconnected.fulfill()
            }
        }
        wait(for: [reconnected], timeout: 1)
    }
}
