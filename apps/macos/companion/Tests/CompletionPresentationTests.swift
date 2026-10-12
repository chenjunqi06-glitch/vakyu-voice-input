//! 终态收起不修改投递结果；旧计时器不得影响重试、恢复查看或新录音。
import XCTest
@testable import VakyuVoice

final class CompletionPresentationTests: XCTestCase {
    func testFailureCollapsesAfterThreeSeconds() {
        var callbacks: [() -> Void] = []
        var delays: [TimeInterval] = []
        let presentation = CompletionPresentation { delay, work in
            delays.append(delay); callbacks.append(work)
        }
        var count = 0
        presentation.presentFailure { count += 1 }
        XCTAssertFalse(presentation.collapsed)
        XCTAssertEqual(delays, [3])
        callbacks[0]()
        XCTAssertTrue(presentation.collapsed)
        XCTAssertEqual(count, 1)
    }

    func testResetPreventsOldTimerFromCollapsingNewRound() {
        var callbacks: [() -> Void] = []
        let presentation = CompletionPresentation { _, work in callbacks.append(work) }
        var count = 0
        presentation.presentFailure { count += 1 }
        presentation.reset()
        callbacks[0]()
        XCTAssertFalse(presentation.collapsed)
        XCTAssertEqual(count, 0)
    }

    func testRetryReplacesOldCompletionTimer() {
        var callbacks: [() -> Void] = []
        let presentation = CompletionPresentation { _, work in callbacks.append(work) }
        var completions: [String] = []
        presentation.presentFailure { completions.append("old") }
        presentation.presentFailure { completions.append("current") }
        callbacks[0]()
        XCTAssertEqual(completions, [])
        callbacks[1]()
        XCTAssertEqual(completions, ["current"])
    }

    func testManualCollapseAndReopenInvalidatePendingTimer() {
        var callbacks: [() -> Void] = []
        let presentation = CompletionPresentation { _, work in callbacks.append(work) }
        var count = 0
        presentation.presentFailure { count += 1 }
        presentation.collapseNow()
        XCTAssertTrue(presentation.collapsed)
        presentation.reset()
        callbacks[0]()
        XCTAssertFalse(presentation.collapsed)
        XCTAssertEqual(count, 0)
    }
}
