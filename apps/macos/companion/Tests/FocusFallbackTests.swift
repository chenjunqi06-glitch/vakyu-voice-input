//! 受限粘贴只接受相同前台应用/窗口，权限和安全拒绝不可被兼容路径掩盖。
import XCTest
@testable import VakyuVoice

final class FocusFallbackTests: XCTestCase {
    func testMissingEditorRequiresEnabledFallbackAndKnownWindow() {
        let focus = FocusResolution<Int>.missing
        func allowed(_ enabled: Bool, _ window: Int?) -> Bool {
            focus.permitsPasteFallback(enabled: enabled, processID: 42, savedWindow: window,
                                       frontmostPID: { 42 }, currentWindow: { 7 }, secureInput: { false })
        }
        XCTAssertTrue(allowed(true, 7))
        XCTAssertFalse(allowed(false, 7))
        XCTAssertFalse(allowed(true, nil))
        XCTAssertFalse(allowed(true, 8))
    }

    func testApplicationSwitchDuringWindowReadIsRejected() {
        var pid: Int32 = 42
        let allowed = FocusResolution<Int>.missing.permitsPasteFallback(
            enabled: true, processID: 42, savedWindow: 7, frontmostPID: { pid },
            currentWindow: { pid = 99; return 7 }, secureInput: { false })
        XCTAssertFalse(allowed)
    }

    func testSecureInputBeforeOrDuringWindowReadIsRejected() {
        for initiallySecure in [false, true] {
            var secure = initiallySecure
            let allowed = FocusResolution<Int>.missing.permitsPasteFallback(
                enabled: true, processID: 42, savedWindow: 7, frontmostPID: { 42 },
                currentWindow: { secure = true; return 7 }, secureInput: { secure })
            XCTAssertFalse(allowed)
        }
    }

    func testPermissionSecureAndForeignFocusNeverPermitFallback() {
        for focus in [FocusResolution<Int>.accessibilityUnavailable, .secureInput, .secureElement, .contextChanged] {
            XCTAssertFalse(focus.permitsPasteFallback(enabled: true, processID: 42, savedWindow: 7,
                frontmostPID: { 42 }, currentWindow: { 7 }, secureInput: { false }))
        }
    }

    func testPermissionErrorStopsBeforeApplicationFocusFallback() {
        var available = true
        var queriedApp = false
        let result = FocusResolution<Int>.resolve(processID: 42, currentProcessID: 1,
            frontmostPID: { 42 }, secureInput: { false },
            systemFocus: { available = false; return nil },
            applicationFocus: { _ in queriedApp = true; return 2 },
            nodePID: { _ in 42 }, isSecure: { _ in false }, accessibilityAvailable: { available })
        XCTAssertEqual(result, .accessibilityUnavailable)
        XCTAssertFalse(queriedApp)
    }
}
