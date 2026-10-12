//! 查询实际生产解析器的内存回归；不访问 AX、真实输入框或键盘。
import XCTest
@testable import VakyuVoice

final class FocusResolutionTests: XCTestCase {
    func testSystemFocusDoesNotQueryApplication() {
        let memory = FocusMemory()
        XCTAssertEqual(memory.resolve(), .element(1))
        XCTAssertEqual(memory.systemReads, 1)
        XCTAssertEqual(memory.applicationQueries, [])
        XCTAssertEqual(memory.sources, [.system])
    }

    func testMissingSystemFocusFallsBackOnlyToExpectedApplication() {
        let memory = FocusMemory()
        memory.system = nil
        XCTAssertEqual(memory.resolve(), .element(2))
        XCTAssertEqual(memory.applicationQueries, [42])
        XCTAssertEqual(memory.sources, [.application])
    }

    func testMissingFocusIsDistinctFromSecure() {
        let memory = FocusMemory()
        memory.system = nil
        memory.application = nil
        XCTAssertEqual(memory.resolve(), .missing)
        XCTAssertNotEqual(memory.resolve(), .secureInput)
        XCTAssertNotEqual(memory.resolve(), .secureElement)
        XCTAssertEqual(memory.applicationQueries, [42, 42, 42])
        XCTAssertEqual(memory.sources, [])
    }

    func testInvalidInitialContextNeverQueriesFocus() {
        for frontmost: Int32? in [nil, 99] {
            let memory = FocusMemory()
            memory.frontmost = frontmost
            XCTAssertEqual(memory.resolve(), .contextChanged)
            XCTAssertEqual(memory.systemReads, 0)
            XCTAssertEqual(memory.applicationQueries, [])
        }
        for processID: Int32 in [0, -1, 7] {
            let memory = FocusMemory()
            memory.frontmost = processID
            XCTAssertEqual(memory.resolve(processID: processID), .contextChanged)
            XCTAssertEqual(memory.systemReads, 0)
            XCTAssertEqual(memory.applicationQueries, [])
        }
    }

    func testForeignOrUnreadableSystemPIDCannotBeHiddenByFallback() {
        for pid: Int32? in [nil, 99, 7] {
            let memory = FocusMemory()
            memory.pids[1] = pid
            XCTAssertEqual(memory.resolve(), .contextChanged)
            XCTAssertEqual(memory.applicationQueries, [])
        }
    }

    func testForeignOrUnreadableApplicationPIDIsRejected() {
        for pid: Int32? in [nil, 99, 7] {
            let memory = FocusMemory()
            memory.system = nil
            memory.pids[2] = pid
            XCTAssertEqual(memory.resolve(), .contextChanged)
            XCTAssertEqual(memory.applicationQueries, [42])
        }
    }

    func testSecureSystemFocusCannotBeHiddenByFallback() {
        let memory = FocusMemory()
        memory.secureNodes.insert(1)
        XCTAssertEqual(memory.resolve(), .secureElement)
        XCTAssertEqual(memory.applicationQueries, [])
        XCTAssertEqual(memory.sources, [.system])
    }

    func testSecureApplicationFocusIsRejected() {
        let memory = FocusMemory()
        memory.system = nil
        memory.secureNodes.insert(2)
        XCTAssertEqual(memory.resolve(), .secureElement)
        XCTAssertEqual(memory.applicationQueries, [42])
        XCTAssertEqual(memory.sources, [.application])
    }

    func testFrontApplicationChangeAfterSystemReadPreventsFallback() {
        for system: Int? in [nil, 1] {
            let memory = FocusMemory()
            memory.system = system
            memory.onSystemRead = { memory.frontmost = 99 }
            XCTAssertEqual(memory.resolve(), .contextChanged)
            XCTAssertEqual(memory.applicationQueries, [])
        }
    }

    func testFrontApplicationChangeAfterFallbackIsNotMissingOrRetargeted() {
        for application: Int? in [nil, 2] {
            let memory = FocusMemory()
            memory.system = nil
            memory.application = application
            memory.onApplicationRead = { memory.frontmost = 99 }
            XCTAssertEqual(memory.resolve(), .contextChanged)
            XCTAssertEqual(memory.applicationQueries, [42])
        }
    }

    func testFrontApplicationIsCheckedAfterCandidateAttributeReads() {
        for usePIDRead in [true, false] {
            let memory = FocusMemory()
            if usePIDRead { memory.onPIDRead = { memory.frontmost = 99 } }
            else { memory.onSecurityRead = { memory.frontmost = nil } }
            XCTAssertEqual(memory.resolve(), .contextChanged)
            XCTAssertEqual(memory.applicationQueries, [])
        }
    }

    func testGlobalSecureInputRejectsBeforeAndAfterFocusQueries() {
        let initial = FocusMemory()
        initial.globalSecure = true
        XCTAssertEqual(initial.resolve(), .secureInput)
        XCTAssertEqual(initial.systemReads, 0)
        for stage in 0..<4 {
            let memory = FocusMemory()
            if stage == 0 { memory.onSystemRead = { memory.globalSecure = true } }
            if stage == 1 {
                memory.system = nil
                memory.onApplicationRead = { memory.globalSecure = true }
            }
            if stage == 2 { memory.onPIDRead = { memory.globalSecure = true } }
            if stage == 3 { memory.onSecurityRead = { memory.globalSecure = true } }
            XCTAssertEqual(memory.resolve(), .secureInput)
            XCTAssertEqual(memory.applicationQueries, stage == 1 ? [42] : [])
        }
    }

    func testSecureInputAfterMissingFallbackIsNotReportedAsMissing() {
        let memory = FocusMemory()
        memory.system = nil
        memory.application = nil
        memory.onApplicationRead = { memory.globalSecure = true }
        XCTAssertEqual(memory.resolve(), .secureInput)
        XCTAssertEqual(memory.applicationQueries, [42])
        XCTAssertEqual(memory.sources, [])
    }

    func testCaptureAndValidationCanUseDifferentSourcesForSameEditor() {
        let memory = FocusMemory()
        memory.system = nil
        let captured = memory.resolve()
        XCTAssertEqual(captured, .element(2))
        memory.system = 2
        XCTAssertEqual(memory.resolve(), captured)
        memory.system = 1
        XCTAssertNotEqual(memory.resolve(), captured)
        XCTAssertEqual(memory.applicationQueries, [42])
    }
}
