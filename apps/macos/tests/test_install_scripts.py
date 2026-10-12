"""在隔离的临时目录注入命令失败，验证登记重试与安装回滚；不触碰系统输入源。"""

import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest


BASE = Path(__file__).resolve().parents[1]


class InstallScriptsTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="vakyu-install-test-")
        self.root = Path(self.temporary.name).resolve()
        self.home = self.root / "user"
        self.system = self.root / "system"
        self.app = self.home / "Library/Input Methods/Vakyu.app"
        self.events = self.root / "events.jsonl"
        self.state = self.root / "state.json"
        self.write_state({})
        self.bin = self.root / "mock-bin"
        self.bin.mkdir()
        for name in ("codesign", "lsregister", "pkill", "open", "launchctl", "ditto", "PlistBuddy", "stat", "id", "dscl"):
            self.mock(self.bin / name, name)

    def tearDown(self):
        self.temporary.cleanup()

    def write_state(self, state):
        self.state.write_text(json.dumps(state))

    def mock(self, path, name):
        path.parent.mkdir(parents=True, exist_ok=True)
        program = f"""#!{sys.executable}
import json, os, pathlib, shutil, subprocess, sys
name = {name!r}
if name.endswith('-verify') and sys.argv[1:2] == ['--current-source']:
    name = 'source-snapshot'
state_path = pathlib.Path({str(self.state)!r})
state = json.loads(state_path.read_text())
count = state.get(name + '_count', 0)
state[name + '_count'] = count + 1
state_path.write_text(json.dumps(state))
with open({str(self.events)!r}, 'a') as stream:
    stream.write(json.dumps({{'name': name, 'args': sys.argv[1:], 'pid': os.getpid()}}) + '\\n')
statuses = state.get(name + '_statuses', [0])
status = statuses[min(count, len(statuses) - 1)]
if status: sys.exit(status)
if name == 'ditto': shutil.copytree(sys.argv[-2], sys.argv[-1], symlinks=True)
if name == 'PlistBuddy': print('app.qingjian.inputmethod')
if name == 'stat': print(state.get('console_user', 'fixture-user'))
if name == 'id': print('501')
if name == 'dscl': print('NFSHomeDirectory: ' + {str(self.home)!r})
if name == 'source-snapshot': print('com.apple.keylayout.ABC')
if name == 'launchctl' and sys.argv[1:2] == ['asuser']:
    command = sys.argv[3:]
    if command[:1] == ['sudo']: command = command[4:]
    sys.exit(subprocess.run(command).returncode)
"""
        path.write_text(program)
        path.chmod(0o755)

    def fixture_app(self, path, version):
        (path / "Contents").mkdir(parents=True)
        (path / "Contents/Info.plist").write_text("fixture metadata")
        (path / "version").write_text(version)
        self.mock(path / "Contents/MacOS/qingjian-macos", version + "-register")
        self.mock(path / "Contents/Resources/verify_input_source", version + "-verify")
        # 被 /bin/sh 调用的脚本启动独立 mock 进程，模拟包内完成登记与校验的入口。
        helper = self.bin / (version + "-finalize")
        self.mock(helper, version + "-finalize")
        (path / "Contents/Resources/register_input_source.sh").write_text(f'#!/bin/sh\nexec "{helper}" "$@"\n')

    def render(self, name, uid="501"):
        text = (BASE / "scripts" / name).read_text()
        text = text.replace("$HOME", str(self.home)).replace("$(id -u)", uid)
        text = text.replace("/Library/Input Methods", str(self.system / "Input Methods"))
        text = text.replace("/Library/Input\\ Methods", str(self.system / "Input\\ Methods"))
        # 前一次替换也经过 user 路径；统一把测试用户安装位置还原为隔离 fixture。
        text = text.replace(str(self.home) + str(self.system / "Input Methods"), str(self.home / "Library/Input Methods"))
        for command in ("codesign", "pkill", "open", "ditto"):
            text = text.replace("/usr/bin/" + command, str(self.bin / command))
        text = text.replace("/bin/launchctl", str(self.bin / "launchctl"))
        text = text.replace("/usr/libexec/PlistBuddy", str(self.bin / "PlistBuddy"))
        text = text.replace("/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister", str(self.bin / "lsregister"))
        text = text.replace("sleep 3", ":").replace("sleep 1", ":")
        script = self.root / name
        script.write_text(text)
        return script

    def run_script(self, script, argument):
        return subprocess.run(["/bin/bash", str(script), str(argument)], capture_output=True, text=True, errors="replace")

    def render_pkg(self, name):
        text = (BASE / "pkg/scripts" / name).read_text()
        text = text.replace("/Library/Input Methods", str(self.system / "Input Methods"))
        text = text.replace("/Library/Input\\ Methods", str(self.system / "Input\\ Methods"))
        # 双引号中的用户路径应仍指向测试用户目录。
        text = text.replace("$user_home" + str(self.system / "Input Methods"), "$user_home/Library/Input Methods")
        for command in ("codesign", "open", "ditto"):
            text = text.replace("/usr/bin/" + command, str(self.bin / command))
        text = text.replace("/usr/libexec/PlistBuddy", str(self.bin / "PlistBuddy"))
        text = text.replace("/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister", str(self.bin / "lsregister"))
        for command in ("stat", "id", "dscl", "launchctl", "pkill"):
            text = text.replace(command + " ", str(self.bin / command) + " ")
        script = self.root / name
        script.write_text(text)
        return script

    def event_names(self):
        if not self.events.exists():
            return []
        return [json.loads(line)["name"] for line in self.events.read_text().splitlines()]

    def registration(self, state=None, uid="501"):
        self.fixture_app(self.app, "current")
        self.write_state(state or {})
        return self.run_script(self.render("register_input_source.sh", uid), self.app)

    def test_registration_requires_two_fresh_successes(self):
        result = self.registration()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.event_names().count("current-register"), 1)
        checks = [json.loads(line) for line in self.events.read_text().splitlines() if json.loads(line)["name"] == "current-verify"]
        self.assertEqual(len(checks), 2)
        self.assertNotEqual(checks[0]["pid"], checks[1]["pid"])

    def test_registration_recovers_from_transient_lookup_failure(self):
        result = self.registration({"current-verify_statuses": [1, 0, 0]})
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.event_names().count("current-register"), 2)

    def test_registration_retries_when_second_lookup_is_invalidated(self):
        result = self.registration({"current-verify_statuses": [0, 1, 0, 0]})
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.event_names().count("current-verify"), 4)

    def test_registration_never_swallows_persistent_failure(self):
        result = self.registration({"current-register_statuses": [1]})
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.event_names().count("current-register"), 2)
        self.assertNotIn("current-verify", self.event_names())

    def test_invalid_signature_stops_before_registration(self):
        result = self.registration({"codesign_statuses": [1]})
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.event_names(), ["codesign"])

    def test_registration_rejects_root(self):
        result = self.registration(uid="0")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.event_names(), [])

    def test_registration_rejects_build_path(self):
        staged = self.root / "Vakyu.app"
        self.fixture_app(staged, "staged")
        result = self.run_script(self.render("register_input_source.sh"), staged)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.event_names(), [])

    def local_install(self, state=None):
        self.fixture_app(self.app, "old")
        source = self.root / "build.noindex/Vakyu.app"
        self.fixture_app(source, "new")
        self.write_state(state or {})
        return self.run_script(self.render("install_local.sh"), source)

    def test_local_install_verifies_before_success(self):
        result = self.local_install()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((self.app / "version").read_text(), "new")
        self.assertIn("new-finalize", self.event_names())
        self.assertEqual(self.event_names()[-1], "new-verify")
        self.assertEqual(list(self.app.parent.glob(".vakyu-install.*")), [])

    def test_local_install_restores_old_app_when_registration_fails(self):
        result = self.local_install({"new-finalize_statuses": [1]})
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual((self.app / "version").read_text(), "old")
        self.assertIn("old-finalize", self.event_names())

    def test_local_install_restores_old_app_when_final_verification_fails(self):
        result = self.local_install({"new-verify_statuses": [1]})
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual((self.app / "version").read_text(), "old")
        self.assertIn("old-finalize", self.event_names())

    def test_local_install_rejects_system_conflict_before_mutation(self):
        self.fixture_app(self.system / "Input Methods/Vakyu.app", "system")
        result = self.local_install()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual((self.app / "version").read_text(), "old")
        self.assertNotIn("pkill", self.event_names())
        self.assertNotIn("ditto", self.event_names())

    def test_local_first_install_failure_restores_previous_selection_before_removal(self):
        source = self.root / "build.noindex/Vakyu.app"
        self.fixture_app(source, "new")
        self.write_state({"new-finalize_statuses": [1]})
        result = self.run_script(self.render("install_local.sh"), source)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(self.app.exists())
        names = self.event_names()
        self.assertLess(names.index("new-register"), names.index("lsregister"))

    def package_install(self, state=None, legacy=False):
        destination = self.system / "Input Methods/Vakyu.app"
        original = destination.with_name("Qingjian.app") if legacy else destination
        self.fixture_app(original, "old")
        self.write_state(state or {})
        before = self.run_script(self.render_pkg("preinstall"), "unused")
        self.assertEqual(before.returncode, 0, before.stderr)
        # 模拟 Installer 写入载荷，不执行 pkgbuild/installer。
        if destination.exists():
            shutil.rmtree(destination)
        self.fixture_app(destination, "new")
        result = self.run_script(self.render_pkg("postinstall"), "unused")
        return result, destination, original

    def test_package_success_requires_fresh_verification(self):
        result, destination, _ = self.package_install()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((destination / "version").read_text(), "new")
        self.assertEqual(self.event_names()[-1], "new-verify")
        self.assertFalse((destination.parent / ".vakyu-pkg-backup").exists())

    def test_package_registration_failure_restores_original(self):
        result, destination, original = self.package_install({"new-finalize_statuses": [1]})
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual((original / "version").read_text(), "old")
        self.assertIn("old-register", self.event_names())
        self.assertFalse((destination.parent / ".vakyu-pkg-backup").exists())

    def test_package_final_verification_failure_restores_original(self):
        result, destination, original = self.package_install({"new-verify_statuses": [1, 0]})
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual((original / "version").read_text(), "old")
        self.assertIn("old-register", self.event_names())
        self.assertFalse((destination.parent / ".vakyu-pkg-backup").exists())

    def test_package_legacy_failure_restores_original_name(self):
        result, destination, original = self.package_install({"new-finalize_statuses": [1]}, legacy=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(destination.exists())
        self.assertEqual((original / "version").read_text(), "old")

    def test_package_rejects_user_install_before_stopping_apps(self):
        self.fixture_app(self.app, "user")
        result = self.run_script(self.render_pkg("preinstall"), "unused")
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("pkill", self.event_names())
        self.assertNotIn("launchctl", self.event_names())
        self.assertFalse((self.system / "Input Methods/.vakyu-pkg-backup").exists())

    def test_package_rejects_missing_desktop_before_mutation(self):
        self.write_state({"console_user": "loginwindow"})
        result = self.run_script(self.render_pkg("preinstall"), "unused")
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("pkill", self.event_names())
        self.assertFalse((self.system / "Input Methods").exists())

    def test_package_first_install_failure_restores_previous_selection(self):
        self.write_state({"new-finalize_statuses": [1]})
        before = self.run_script(self.render_pkg("preinstall"), "unused")
        self.assertEqual(before.returncode, 0, before.stderr)
        destination = self.system / "Input Methods/Vakyu.app"
        self.fixture_app(destination, "new")
        result = self.run_script(self.render_pkg("postinstall"), "unused")
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(destination.exists())
        self.assertIn("new-register", self.event_names())
        self.assertIn("lsregister", self.event_names())

    def test_package_backup_failure_keeps_original_and_cleans_partial_backup(self):
        destination = self.system / "Input Methods/Vakyu.app"
        self.fixture_app(destination, "old")
        self.write_state({"codesign_statuses": [1]})
        result = self.run_script(self.render_pkg("preinstall"), "unused")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual((destination / "version").read_text(), "old")
        self.assertFalse((destination.parent / ".vakyu-pkg-backup").exists())
        self.assertNotIn("pkill", self.event_names())


if __name__ == "__main__":
    unittest.main()
