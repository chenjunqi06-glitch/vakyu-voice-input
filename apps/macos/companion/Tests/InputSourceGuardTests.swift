// 输入法登记自愈只认正式安装目录；不调用 TIS 登记、不修改系统输入源。
import XCTest
@testable import VakyuVoice

final class InputSourceGuardTests: XCTestCase {
    func testBuildCopiesAreNeverRegistered() {
        let helper = URL(fileURLWithPath: "/tmp/vakyu-build.noindex/Vakyu.app/Contents/Resources/VakyuVoice.app")
        XCTAssertNil(InputSourceGuard.installedInputMethod(helper: helper))
    }

    func testOnlyNestedHelperLayoutResolvesToOuterBundle() {
        let loose = URL(fileURLWithPath: "/Library/Input Methods/VakyuVoice.app")
        XCTAssertNil(InputSourceGuard.installedInputMethod(helper: loose))
    }

    func testModeBelongsToParentSource() {
        XCTAssertTrue(InputSourceGuard.modeID.hasPrefix(InputSourceGuard.parentID + "."))
    }
}
