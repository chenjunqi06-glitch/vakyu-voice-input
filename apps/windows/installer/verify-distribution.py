"""验证离线分发内容与独立 Python 语音握手；不开麦、不改变已安装程序。"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import queue
import re
import struct
import subprocess
import threading
import time
import tomllib


def sha256(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def pe_machine(path):
    with path.open("rb") as stream:
        assert stream.read(2) == b"MZ", path
        stream.seek(0x3C)
        offset = struct.unpack("<I", stream.read(4))[0]
        stream.seek(offset)
        assert stream.read(4) == b"PE\0\0", path
        return struct.unpack("<H", stream.read(2))[0]


def pe_subsystem(path):
    with path.open("rb") as stream:
        stream.seek(0x3C)
        offset = struct.unpack("<I", stream.read(4))[0]
        stream.seek(offset + 24 + 68)
        return struct.unpack("<H", stream.read(2))[0]


def check_payload(root):
    manifest = json.loads((root / "distribution-manifest.json").read_text(encoding="utf-8-sig"))
    assert manifest["ui_access"] is False
    checked = set()
    total = 0
    for entry in manifest["files"]:
        path = root / entry["path"]
        assert path.resolve().is_relative_to(root.resolve()), entry["path"]
        assert path.stat().st_size == entry["bytes"], entry["path"]
        assert sha256(path) == entry["sha256"], entry["path"]
        checked.add(path.resolve())
        total += entry["bytes"]
    actual = {p.resolve() for p in root.rglob("*") if p.is_file() and p.name != "distribution-manifest.json"}
    assert actual == checked, "payload 文件与哈希清单不一致"
    assert pe_machine(root / "qingjian-server.exe") == 0x8664
    assert pe_subsystem(root / "qingjian-server.exe") == 2, "后台必须是无控制台的 GUI 程序"
    assert pe_subsystem(root / "voice-runtime/python/pythonw.exe") == 2
    assert (root / "voice-runtime/app/dictation_context.py").stat().st_size > 0
    assert pe_machine(root / "qingjian-settings.exe") == 0x8664
    assert pe_machine(root / f"qingjian_tsf-{manifest['version']}.dll") == 0x8664
    assert pe_machine(root / f"qingjian_tsf-{manifest['version']}-x86.dll") == 0x14C
    server = (root / "qingjian-server.exe").read_bytes()
    assert re.search(br'uiAccess\s*=\s*[\"\x27]false[\"\x27]', server), "不应依赖本机 uiAccess 证书"
    forbidden = []
    # 必须是独立凭据前缀，避免把微软 DLL 中的 ...Task-Text-Performance 事件名当作 sk- 密钥。
    credential = re.compile(br"(?<![A-Za-z0-9_-])(?:sk-[A-Za-z0-9_-]{24,}|ghp_[A-Za-z0-9]{30,}|-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----)")
    for path in actual:
        if path.name.lower() in {"config.toml", ".env"} or path.suffix.lower() in {".pfx", ".p12", ".wav", ".log"}:
            forbidden.append(str(path.relative_to(root)))
        if path.suffix.lower() in {".exe", ".dll", ".py", ".json", ".toml", ".txt", ".md", ".lock"}:
            if credential.search(path.read_bytes()):
                forbidden.append(str(path.relative_to(root)))
    assert not forbidden, "需人工复核可能的个人数据/凭据文件：" + ", ".join(forbidden)
    return {"version": manifest["version"], "files": len(checked), "bytes": total, "manifest": "passed", "credential_scan": "passed"}


def clean_environment(test_home):
    for name in ("Roaming", "Local", "Temp"):
        (test_home / name).mkdir(parents=True, exist_ok=True)
    system = os.environ.get("SystemRoot", r"C:\Windows")
    return {
        "SystemRoot": system, "WINDIR": system, "PATH": str(Path(system) / "System32"),
        "USERPROFILE": str(test_home), "APPDATA": str(test_home / "Roaming"),
        "LOCALAPPDATA": str(test_home / "Local"), "TEMP": str(test_home / "Temp"),
        "TMP": str(test_home / "Temp"),
        # 验证其他音频软件遗留的变量不会要求安装未分发的 ASIO 后端。
        "SD_ENABLE_ASIO": "1",
    }


def server_loader(root, test_home):
    # 此入口在非已注册安装目录只生成配置并退出，不启用真实键盘、不接管全局后台。
    outcome = subprocess.run([str(root / "qingjian-server.exe"), "--enable-user-profile"],
                             cwd=test_home, env=clean_environment(test_home), timeout=30,
                             creationflags=0x08000000, capture_output=True)
    assert outcome.returncode == 0, outcome.returncode
    config_file = test_home / "Roaming/Qingjian/config.toml"
    config = tomllib.loads(config_file.read_text(encoding="utf-8-sig"))
    shortcut = config.get("shortcut", {})
    predict = config.get("predict", {})
    assert shortcut.get("voice_input") is True
    assert shortcut.get("voice_english_practice") is False
    assert predict.get("enabled") is False
    assert not predict.get("api_key")
    return {"loader_exit_code": 0, "voice_input": True, "practice": False,
            "cloud_prediction": False, "bundled_api_key": False}


def voice_handshake(root, test_home):
    runtime = root / "voice-runtime"
    before = {str(p.relative_to(runtime)): sha256(p) for p in runtime.rglob("*") if p.is_file()}
    environment = clean_environment(test_home)
    started = time.monotonic()
    process = subprocess.Popen([
        str(runtime / "python/pythonw.exe"), "-I", "-B", "-u", "-X", "utf8",
        str(runtime / "app/main.py"), "--models", str(runtime / "models"),
    ], cwd=test_home, env=environment, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, text=True, encoding="utf-8", creationflags=0x08000000)
    events = queue.Queue()

    def read_stdout():
        for line in process.stdout:
            events.put(line)
        events.put(None)

    threading.Thread(target=read_stdout, daemon=True).start()
    try:
        deadline = time.monotonic() + 120
        while time.monotonic() < deadline:
            line = events.get(timeout=max(0.01, deadline - time.monotonic()))
            assert line is not None, "语音进程在就绪前退出"
            event = json.loads(line)
            assert event.get("event") != "error", event
            if event.get("event") == "ready":
                ready_seconds = round(time.monotonic() - started, 2)
                break
        else:
            raise AssertionError("语音模型未在 120 秒内就绪")
        process.stdin.write('{"command":"shutdown","generation":0}\n')
        process.stdin.flush()
        process.wait(timeout=15)
        assert process.returncode == 0, process.returncode
    finally:
        if process.poll() is None:
            process.kill()
            process.wait(timeout=10)
        process.stdin.close()
        process.stdout.close()
        process.stderr.close()
    after = {str(p.relative_to(runtime)): sha256(p) for p in runtime.rglob("*") if p.is_file()}
    assert before == after, "语音组件写入了安装目录"
    return {"ready_seconds": ready_seconds, "shutdown": "passed", "runtime_unchanged": True,
            "microphone_opened": False, "clean_environment": True}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--payload", required=True, type=Path)
    parser.add_argument("--test-home", required=True, type=Path)
    parser.add_argument("--report", required=True, type=Path)
    args = parser.parse_args()
    result = check_payload(args.payload.resolve())
    result["defaults"] = server_loader(args.payload.resolve(), args.test_home.resolve())
    result["voice"] = voice_handshake(args.payload.resolve(), args.test_home.resolve())
    result["limitation"] = "独立目录/空环境验证，不等于全新 Windows 安装/卸载或真实麦克风及各应用验收"
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(result, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
