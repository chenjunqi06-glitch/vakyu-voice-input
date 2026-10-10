"""检查 Apple Silicon 移植清单；只有 --bundle 才验证实际生成的 app。"""

import argparse
import json
from pathlib import Path
import plistlib
import platform
import re
import subprocess
import xml.etree.ElementTree as ET

from distribution_policy import check_credentials, private_path

ROOT = Path(__file__).resolve().parents[3]


def check_source():
    base = ROOT / "apps/macos"
    for path in [base / "Info.plist", *(base / "resources").glob("*.plist"),
                 base / "resources/Voice.entitlements"]:
        with path.open("rb") as source:
            plistlib.load(source)
    ET.parse(base / "pkg/distribution.xml")
    for path in [*(base / "scripts").glob("*.sh"), *(base / "pkg/scripts").iterdir()]:
        if path.is_file():
            assert b"\r" not in path.read_bytes(), f"shell 文件包含 CRLF：{path}"
    sources = json.loads((base / "voice-runtime/sources.json").read_text())
    for item in [sources["python"], sources["model"], *sources["wheels"]]:
        assert re.fullmatch(r"[a-f0-9]{64}", item["sha256"])
        assert item["url"].startswith("https://")
    assert sources["architecture"] == "arm64"
    print("源码 plist/XML、LF shell、下载来源与 SHA256 清单检查通过")


def check_bundle(bundle):
    resources = bundle / "Contents/Resources"
    required = [bundle / "Contents/MacOS/qingjian-macos", resources / "Vakyu.icns",
                resources / "vakyu-icon.png", resources / "vakyu-logo.png",
                resources / "install_voice_agent.sh", resources / "VoiceLaunchAgent.plist"]
    helper = resources / "VakyuVoice.app"
    required += [helper / "Contents/MacOS/VakyuVoice", helper / "Contents/Info.plist"]
    nested = helper / "Contents/Resources"
    required += [nested / "vakyu-voice-service", nested / "vakyu-icon.png", nested / "vakyu-logo.png"]
    runtime = nested / "voice-runtime"
    required += [runtime / "python/bin/python3", runtime / "sources.json",
                 runtime / "app/external_main.py", runtime / "app/recognition.py",
                 runtime / "app/channel.py", runtime / "app/pcm.py",
                 runtime / "models/streaming/encoder.int8.onnx",
                 runtime / "models/streaming/decoder.onnx",
                 runtime / "models/streaming/joiner.int8.onnx",
                 runtime / "models/streaming/tokens.txt", runtime / "models/streaming/bpe.vocab"]
    for path in required:
        assert path.is_file() and path.stat().st_size > 0, f"缺失组件：{path}"
    if platform.system() == "Darwin":
        for binary in [required[0], helper / "Contents/MacOS/VakyuVoice",
                       nested / "vakyu-voice-service", runtime / "python/bin/python3"]:
            subprocess.run(["lipo", "-verify_arch", "arm64", str(binary)], check=True)
        for signed in (helper, bundle):
            subprocess.run(["codesign", "--verify", "--deep", "--strict", str(signed)], check=True)
        # Resources 下的 Python 及动态库需要逐个验签；不能只依赖外壳资源封印。
        for path in runtime.rglob("*"):
            if not path.is_file() or path.is_symlink():
                continue
            kind = subprocess.check_output(["file", "-b", str(path)], text=True)
            if "Mach-O" in kind:
                subprocess.run(["codesign", "--verify", "--strict", str(path)], check=True)
    for path in bundle.rglob("*"):
        if not path.is_file():
            continue
        relative = path.relative_to(bundle).as_posix()
        assert not private_path(relative), f"安装包包含个人资料路径：{relative}"
        assert path.suffix.lower() != ".dll", f"Apple 包包含 Windows DLL：{relative}"
        if path.suffix.lower() in {".json", ".toml", ".py", ".sh", ".plist", ".txt", ".md"}:
            check_credentials(path.read_bytes(), relative)
    print("实际 app 资源、运行时、模型与隐私排除清单检查通过")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--bundle", type=Path)
    args = parser.parse_args()
    check_source()
    if args.bundle:
        check_bundle(args.bundle)
