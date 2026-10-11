"""准备可重定位的 Apple Silicon 语音运行时；下载与模型均按 SHA256 锁定。"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import tarfile
import tempfile
import urllib.request

ROOT = Path(__file__).resolve().parents[3]
RUNTIME = ROOT / "apps/macos/voice-runtime"
SHARED = ROOT / "apps/windows/server/voice-runtime"


def digest(path):
    value = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            value.update(chunk)
    return value.hexdigest()


def download(item, cache):
    target = cache / item["filename"]
    if target.is_file() and digest(target) == item["sha256"]:
        return target
    partial = target.with_suffix(target.suffix + ".partial")
    print(f"下载 {item['filename']}", flush=True)
    try:
        with urllib.request.urlopen(item["url"], timeout=90) as response, partial.open("wb") as output:
            shutil.copyfileobj(response, output, 1024 * 1024)
        if digest(partial) != item["sha256"]:
            raise ValueError(f"SHA256 校验失败：{item['filename']}")
        partial.replace(target)
    finally:
        partial.unlink(missing_ok=True)
    return target


def unpack(archive, destination):
    destination = destination.resolve()
    destination.mkdir(parents=True, exist_ok=True)
    with tarfile.open(archive) as source:
        for member in source.getmembers():
            target = (destination / member.name).resolve()
            if not target.is_relative_to(destination) or member.isdev():
                raise ValueError("压缩包包含越界路径或设备")
            if member.issym() or member.islnk():
                parent = target.parent if member.issym() else destination
                linked = (parent / member.linkname).resolve()
                if not linked.is_relative_to(destination):
                    raise ValueError("压缩包包含越界链接")
        # 所有路径与链接已检查；兼容 Xcode 自带的 Python 3.9。
        source.extractall(destination)


def prepare(destination, cache):
    if platform.system() != "Darwin" or platform.machine() != "arm64":
        raise SystemExit("完整语音运行时需要在 Apple 芯片 Mac 原生构建，不能复制 Windows DLL")
    manifest = json.loads((RUNTIME / "sources.json").read_text(encoding="utf-8"))
    cache.mkdir(parents=True, exist_ok=True)
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="vakyu-runtime-", dir=destination.parent) as temporary:
        staging = Path(temporary) / "runtime"
        staging.mkdir()
        unpack(download(manifest["python"], cache), staging)
        python = staging / "python/bin/python3"
        wheels = [download(item, cache) for item in manifest["wheels"]]
        environment = {**os.environ, "PYTHONDONTWRITEBYTECODE": "1", "PYTHONNOUSERSITE": "1"}
        environment.pop("PYTHONPATH", None)
        environment.pop("PYTHONHOME", None)
        subprocess.run([str(python), "-m", "pip", "install", "--no-index", "--no-deps",
                        "--disable-pip-version-check", *map(str, wheels)], env=environment, check=True)
        # 独立 Python 的标准库测试可能带示例录音/测试证书；运行服务不需要这些资料。
        standard_tests = staging / "python/lib/python3.12/test"
        if standard_tests.is_dir():
            shutil.rmtree(standard_tests)
        unpacked = Path(temporary) / "model"
        unpack(download(manifest["model"], cache), unpacked)
        tokens = list(unpacked.rglob("tokens.txt"))
        if len(tokens) != 1:
            raise ValueError("模型包必须包含唯一的 streaming 词表")
        model = staging / "models/streaming"
        model.mkdir(parents=True)
        # 上游包另带 test_wavs，运行版只复制必需模型，不能把示例录音装给用户。
        for filename in ("encoder.int8.onnx", "decoder.onnx", "joiner.int8.onnx", "tokens.txt", "README.md"):
            shutil.copy2(tokens[0].parent / filename, model / filename)
        app = staging / "app"
        app.mkdir()
        for filename in ("external_main.py", "channel.py", "pcm.py"):
            shutil.copy2(RUNTIME / filename, app / filename)
        for filename in ("recognition.py", "practice_context.py", "dictation_context.py", "bpe_vocab.py", "audio_meter.py"):
            shutil.copy2(SHARED / filename, app / filename)
        shutil.copy2(RUNTIME / "sources.json", staging / "sources.json")
        shutil.copy2(RUNTIME / "README.md", staging / "README.md")
        subprocess.run([str(python), str(app / "external_main.py"), "--models",
                        str(staging / "models"), "--check-model"], env=environment, check=True)
        # 热词词表预先生成；安装后签名目录只读，不在签名包中写入缓存。
        for folder in staging.rglob("__pycache__"):
            shutil.rmtree(folder)
        inventory = {str(path.relative_to(staging)): digest(path)
                     for path in staging.rglob("*") if path.is_file() and not path.is_symlink()}
        (staging / "SHA256SUMS.json").write_text(json.dumps(inventory, indent=2) + "\n", encoding="utf-8")
        if destination.exists():
            shutil.rmtree(destination)
        staging.rename(destination)
    print(f"已准备 Apple Silicon 语音运行时：{destination}", flush=True)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=ROOT / "target/macos-voice-runtime/arm64")
    parser.add_argument("--cache", type=Path, default=ROOT / "target/macos-download-cache")
    args = parser.parse_args()
    prepare(args.output.resolve(), args.cache.resolve())


if __name__ == "__main__":
    main()
