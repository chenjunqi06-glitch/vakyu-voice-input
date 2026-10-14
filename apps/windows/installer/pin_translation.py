"""一次性固定离线翻译组件：解析运行时依赖的 Windows wheel、把官方 OPUS-MT 权重转换成 CTranslate2 int8，
并把下载地址、SHA256、模型版本写回 translation-sources.json / translation-models.json。

在能上网的 Windows 构建机上用普通 Python 3.12 运行一次（不是随包的嵌入式 Python）：

    py -3.12 apps/windows/installer/pin_translation.py --cache C:/vakyu-cache

转换结果放在 <cache>/translate-models/，prepare-runtime.ps1 用同一个 --cache 目录取用并按清单校验。
之后提交两个 JSON 文件；以后的构建只校验，不再转换。不读取任何个人配置或用户数据。
"""

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
import urllib.request
import venv
from pathlib import Path

HERE = Path(__file__).resolve().parent
SOURCES = HERE / "translation-sources.json"
MODELS = HERE / "translation-models.json"
VOICE_SOURCES = HERE / "voice-sources.json"
MODEL_FILES = ("model.bin", "shared_vocabulary.json", "source_vocabulary.json",
               "target_vocabulary.json", "config.json", "source.spm", "target.spm")


def sha256(path):
    digest = hashlib.sha256()
    with open(path, "rb") as source:
        for block in iter(lambda: source.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def fetch_json(url):
    with urllib.request.urlopen(url, timeout=60) as response:
        return json.load(response)


def run(*command):
    print("+", " ".join(str(part) for part in command), flush=True)
    # 固定模型时不再执行与本次下载无关的 pip 自更新检查。
    environment = dict(os.environ, PIP_DISABLE_PIP_VERSION_CHECK="1")
    subprocess.run([str(part) for part in command], check=True, env=environment)


def pin_packages(sources, work):
    """按嵌入式 Python 3.12 x64 解析依赖；随包已有的 numpy 等不重复打包。"""
    voice_packages = json.loads(VOICE_SOURCES.read_text(encoding="utf-8"))["packages"]
    already = {item["name"].lower().replace("_", "-") for item in voice_packages}
    # 使用随包已固定的 NumPy wheel，避免重新遍历整份跨平台 NumPy 索引。
    bundled_numpy = [item["url"] for item in voice_packages if item["name"].lower() == "numpy"]
    wheels = work / "wheels"
    run(sys.executable, "-m", "pip", "download", "--only-binary=:all:", "--platform", "win_amd64",
        "--python-version", "3.12", "--implementation", "cp", "-d", wheels,
        *sources["requirements"], *bundled_numpy)
    packages = []
    for wheel in sorted(wheels.glob("*.whl")):
        name, version = wheel.name.split("-")[:2]
        normalized = name.lower().replace("_", "-")
        if normalized in already:
            continue
        release = fetch_json(f"https://pypi.org/pypi/{normalized}/{version}/json")
        match = next(item for item in release["urls"] if item["filename"] == wheel.name)
        digest = sha256(wheel)
        if digest != match["digests"]["sha256"]:
            raise SystemExit(f"下载的 {wheel.name} 与 PyPI 记录的哈希不一致")
        packages.append({"url": match["url"], "sha256": digest, "name": normalized, "version": version})
    return packages


def convert_models(sources, cache, work):
    """在临时虚拟环境里装转换工具，把固定版本的官方权重转成 int8。"""
    environment = work / "converter"
    venv.EnvBuilder(with_pip=True).create(environment)
    python = environment / ("Scripts/python.exe" if sys.platform == "win32" else "bin/python")
    # 直接使用 PyPI 官方 wheel，避免 pip 在大量历史版本索引中长时间解析。
    pip_release = fetch_json("https://pypi.org/pypi/pip/json")
    pip_wheel = next(item for item in pip_release["urls"] if item["filename"].endswith("-py3-none-any.whl"))
    run(python, "-m", "pip", "install", "--upgrade", pip_wheel["url"])
    run(python, "-m", "pip", "install", "--extra-index-url", "https://download.pytorch.org/whl/cpu",
        *sources["converter"])
    converter = python.parent / ("ct2-transformers-converter.exe" if sys.platform == "win32"
                                 else "ct2-transformers-converter")
    output_root = cache / "translate-models"
    if output_root.exists():
        shutil.rmtree(output_root)
    manifest = []
    for model in sources["models"]:
        if not model.get("revision"):
            model["revision"] = fetch_json(f"https://huggingface.co/api/models/{model['repo']}")["sha"]
        output = output_root / model["folder"]
        run(converter, "--model", model["repo"], "--revision", model["revision"], "--output_dir", output,
            "--quantization", "int8", "--copy_files", "source.spm", "target.spm", "--force")
        for name in MODEL_FILES:
            path = output / name
            if path.is_file():
                manifest.append({"size": path.stat().st_size, "sha256": sha256(path),
                                 "path": f"models/translate/{model['folder']}/{name}"})
    return manifest


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--cache", type=Path, required=True)
    args = parser.parse_args()
    cache = args.cache.resolve()
    cache.mkdir(parents=True, exist_ok=True)
    sources = json.loads(SOURCES.read_text(encoding="utf-8"))
    with tempfile.TemporaryDirectory() as temporary:
        work = Path(temporary)
        sources["packages"] = pin_packages(sources, work)
        manifest = convert_models(sources, cache, work)
    SOURCES.write_text(json.dumps(sources, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    MODELS.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"已固定 {len(sources['packages'])} 个 wheel、{len(manifest)} 个模型文件；请提交两个 JSON。")


if __name__ == "__main__":
    main()
