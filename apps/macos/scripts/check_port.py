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
from runtime_manifest import verify_manifest

ROOT = Path(__file__).resolve().parents[3]


def check_microphone_description(path):
    with path.open("rb") as source:
        info = plistlib.load(source)
    description = info.get("NSMicrophoneUsageDescription")
    assert isinstance(description, str) and description.strip(), f"缺少麦克风用途说明：{path}"


def check_input_source_icon(path):
    with path.open("rb") as source:
        info = plistlib.load(source)
    expected = "vakyu-input-sourceTemplate.pdf"
    assert info.get("tsInputMethodIconFileKey") == expected, f"输入源须使用独立 PDF 图标：{path}"
    assert info.get("TISIconIsTemplate") is True, f"输入源须使用适应明暗显示的轮廓图标：{path}"
    for mode in info["ComponentInputModeDict"]["tsInputModeListKey"].values():
        for key in ("tsInputModeMenuIconFileKey", "tsInputModeAlternateMenuIconFileKey"):
            assert mode.get(key) == expected, f"输入模式图标不一致：{path} {key}"
        assert mode.get("tsInputModePaletteIconFileKey") == "vakyu-input-source-baseline.pdf"


def check_source():
    base = ROOT / "apps/macos"
    for path in [base / "Info.plist", *(base / "resources").glob("*.plist"),
                 base / "resources/Voice.entitlements"]:
        with path.open("rb") as source:
            plistlib.load(source)
    # 嵌套 app 的请求可能归属于外层 app；两个身份都必须声明用途。
    for path in (base / "Info.plist", base / "resources/VoiceInfo.plist"):
        check_microphone_description(path)
    check_input_source_icon(base / "Info.plist")
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
                resources / "vakyu-icon.png", resources / "vakyu-logo.png", resources / "vakyu-menu.png",
                resources / "vakyu-input-source-baseline.pdf", resources / "vakyu-input-sourceTemplate.pdf",
                resources / "dict.qj", resources / "lm.qj",
                resources / "models/hanzhang-zhiwei/hanzhang-zhiwei-small.qjm",
                resources / "models/hanzhang-tongbian/hanzhang-tongbian-small.qjm",
                resources / "install_voice_agent.sh", resources / "VoiceLaunchAgent.plist",
                resources / "register_input_source.sh", resources / "verify_input_source"]
    helper = resources / "VakyuVoice.app"
    for app in (bundle, helper):
        check_microphone_description(app / "Contents/Info.plist")
    check_input_source_icon(bundle / "Contents/Info.plist")
    required += [helper / "Contents/MacOS/VakyuVoice", helper / "Contents/Info.plist"]
    nested = helper / "Contents/Resources"
    required += [nested / "vakyu-voice-service", nested / "vakyu-icon.png", nested / "vakyu-logo.png",
                 nested / "vakyu-menu.png"]
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
    for name in ("vakyu-input-source-baseline.pdf", "vakyu-input-sourceTemplate.pdf"):
        assert (resources / name).read_bytes().startswith(b"%PDF-"), f"输入源图标不是 PDF：{name}"
    verify_manifest(runtime)
    if platform.system() == "Darwin":
        for binary in [required[0], helper / "Contents/MacOS/VakyuVoice",
                       nested / "vakyu-voice-service", runtime / "python/bin/python3"]:
            subprocess.run(["lipo", str(binary), "-verify_arch", "arm64"], check=True)
        for signed in (helper, bundle):
            subprocess.run(["codesign", "--verify", "--deep", "--strict", str(signed)], check=True)
        # Resources 下的 Python 及动态库需要逐个验签；不能只依赖外壳资源封印。
        for path in runtime.rglob("*"):
            if not path.is_file() or path.is_symlink():
                continue
            kind = subprocess.check_output(["file", "-b", str(path)], text=True)
            if "Mach-O" in kind:
                subprocess.run(["lipo", str(path), "-verify_arch", "arm64"], check=True)
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
