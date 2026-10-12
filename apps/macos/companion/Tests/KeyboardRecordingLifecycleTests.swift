//! 快捷键录入结束只清理自身状态，不吞首次按键，也不让残余键开启录音。
import XCTest
import ApplicationServices
@testable import VakyuVoice

final class KeyboardRecordingLifecycleTests: XCTestCase {
    private let option = CGEventFlags(rawValue: CGEventFlags.maskAlternate.rawValue | 0x20)

    func testClosingIdleSettingsPreservesFirstPressAndActiveShortcut() {
        let source = KeyboardMonitorFakeSource()
        let monitor = KeyboardMonitor(events: source, keyState: { _ in false })
        XCTAssertTrue(monitor.start())
        monitor.endRecording()
        source.send(.flagsChanged, code: 58, flags: option)
        XCTAssertEqual(monitor.heldMode, .dictation)
        monitor.endRecording()
        XCTAssertEqual(monitor.heldMode, .dictation)
        source.send(.flagsChanged, code: 58)
        XCTAssertNil(monitor.heldMode)
    }

    func testCancelEmptyRecordingDoesNotSwallowFirstPress() {
        let source = KeyboardMonitorFakeSource()
        let monitor = KeyboardMonitor(events: source, keyState: { _ in false })
        XCTAssertTrue(monitor.start())
        monitor.beginRecording()
        monitor.endRecording()
        XCTAssertFalse(monitor.recording)
        source.send(.flagsChanged, code: 58, flags: option)
        XCTAssertEqual(monitor.heldMode, .dictation)
    }

    func testCompletedModifierRecordingDoesNotSwallowFollowingPress() throws {
        let source = KeyboardMonitorFakeSource()
        let monitor = KeyboardMonitor(events: source, keyState: { _ in false })
        var saved: Shortcut?
        monitor.onRecorded = { if case .success(let shortcut) = $0 { saved = shortcut } }
        XCTAssertTrue(monitor.start())
        monitor.beginRecording()
        source.send(.flagsChanged, code: 58, flags: option)
        XCTAssertTrue(monitor.recording)
        source.send(.flagsChanged, code: 58)
        XCTAssertEqual(saved, try Shortcut("left_alt"))
        XCTAssertFalse(monitor.recording)
        source.send(.flagsChanged, code: 58, flags: option)
        XCTAssertEqual(monitor.heldMode, .dictation)
    }

    func testCompletedOrdinaryChordWaitsForEveryResidualKey() throws {
        let source = KeyboardMonitorFakeSource()
        var query: Set<UInt16> = []
        let monitor = KeyboardMonitor(events: source, keyState: { query.contains($0) })
        var saved: Shortcut?
        monitor.onRecorded = { if case .success(let shortcut) = $0 { saved = shortcut } }
        XCTAssertTrue(monitor.start())
        monitor.beginRecording()
        query = [55]
        source.send(.flagsChanged, code: 55, flags: [.maskCommand])
        query = [55, 9]
        source.send(.keyDown, code: 9, flags: [.maskCommand])
        XCTAssertEqual(saved, try Shortcut("win+v"))
        XCTAssertFalse(monitor.recording)
        source.send(.keyUp, code: 9, flags: [.maskCommand])
        source.send(.flagsChanged, code: 58, flags: option.union(.maskCommand))
        source.send(.flagsChanged, code: 55, flags: option)
        XCTAssertNil(monitor.heldMode)
        source.send(.flagsChanged, code: 58)
        source.send(.flagsChanged, code: 58, flags: option)
        XCTAssertEqual(monitor.heldMode, .dictation)
    }

    func testInterruptedEmptyRecordingCannotActivateKeyHeldDuringReconnect() throws {
        let source = KeyboardMonitorFakeSource()
        var query: Set<UInt16> = []
        let monitor = KeyboardMonitor(events: source, keyState: { query.contains($0) })
        monitor.configure(enabled: true, practiceEnabled: true, dictationKey: "left_alt", shortcut: try Shortcut("f8"))
        var failures = 0
        monitor.onRecorded = { if case .failure = $0 { failures += 1 } }
        XCTAssertTrue(monitor.start())
        monitor.beginRecording()
        source.available = false
        query = [100]
        XCTAssertTrue(monitor.start())
        XCTAssertFalse(monitor.recording)
        XCTAssertEqual(failures, 1)
        source.send(.keyDown, code: 100, flags: [.maskSecondaryFn])
        XCTAssertNil(monitor.heldMode)
        source.send(.keyUp, code: 100, flags: [.maskSecondaryFn])
        source.send(.keyDown, code: 100, flags: [.maskSecondaryFn])
        XCTAssertEqual(monitor.heldMode, .practice)
    }
}
