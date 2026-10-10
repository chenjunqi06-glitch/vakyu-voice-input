// 按键配置回归，不安装系统钩子、不修改用户快捷键。
import XCTest
@testable import VakyuVoice

final class ShortcutTests: XCTestCase {
    func testRightSidedModifierChordPreservesSides() throws {
        let shortcut = try Shortcut("right_ctrl+right_alt")
        XCTAssertTrue(shortcut.rightControlOption)
        XCTAssertTrue(shortcut.modifierOnly)
        XCTAssertEqual(shortcut.canonical, "right_ctrl+right_alt")
        XCTAssertEqual(shortcut.keyCode, 61)
        XCTAssertEqual(shortcut.label, "右 Control＋右 Option")
    }

    func testNormalizesCrossPlatformNames() throws {
        let shortcut = try Shortcut(" Control + Option + e ")
        XCTAssertEqual(shortcut.canonical, "ctrl+alt+e")
        XCTAssertEqual(shortcut.keyCode, 14)
        XCTAssertEqual(shortcut.modifiers, 3)
        XCTAssertFalse(shortcut.modifierOnly)
        XCTAssertEqual(try Shortcut("command+shift+f8").canonical, "shift+win+f8")
    }

    func testRejectsOrdinaryTypingAndSystemCommands() {
        for value in ["a", "9", "space", "shift+a", "ctrl", "alt+right_alt", "f12", "win+q", "f25"] {
            XCTAssertThrowsError(try Shortcut(value), value)
        }
    }

    func testAcceptsFunctionKeysAndStandaloneOption() throws {
        XCTAssertEqual(try Shortcut("f8").keyCode, 100)
        XCTAssertTrue(try Shortcut("right_alt").modifierOnly)
        XCTAssertTrue(try Shortcut("left_alt").modifierOnly)
        XCTAssertEqual(try Shortcut("ctrl+space").keyCode, 49)
    }
}
