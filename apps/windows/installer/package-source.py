"""归档本次修改源码及锁定的 Windows 依赖源码，不包含构建目录或用户数据。"""

import argparse
import hashlib
import json
from pathlib import Path
import re
import tomllib
import zipfile


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--dependency-index", type=Path, required=True)
    parser.add_argument("--cargo-cache", type=Path, required=True)
    parser.add_argument("--cargo-metadata", type=Path, required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[3]
    dirs = ["apps", "assets", "crates", "docs", "tools", ".cargo", ".githooks", ".github"]
    files = []
    for folder in dirs:
        for path in (root / folder).rglob("*"):
            if not path.is_file() or path.is_symlink():
                continue
            relative = path.relative_to(root)
            if any(part in {"target", "outputs", "node_modules", "__pycache__", ".git", ".venv", "build", ".build"} for part in relative.parts):
                continue
            if path.suffix in {".pyc", ".pyo", ".bak", ".old", ".log"}:
                continue
            if path.suffix.lower() in {".pfx", ".p12", ".key", ".pem", ".dmp", ".wav", ".mp3", ".sqlite", ".sqlite3"}:
                raise ValueError(f"拒绝归档疑似私人文件：{relative}")
            if path.name == ".env" or path.name.startswith(".env.") and path.name != ".env.example":
                raise ValueError(f"拒绝归档私人环境文件：{relative}")
            files.append(path)
    for name in ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "LICENSE", "README.md", "README.en.md", "UPSTREAM.md", "CHANGELOG.md", "SECURITY.md", "CONTRIBUTING.md", "AGENTS.md", "CLAUDE.md", ".gitignore", ".gitattributes", ".env.example"]:
        path = root / name
        if path.is_file():
            files.append(path)
    private = re.compile(br"sk-[A-Za-z0-9_-]{24,}|gh[pousr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{30,}|-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----")
    for path in files:
        if private.search(path.read_bytes()):
            raise ValueError(f"源码含疑似凭据，请人工检查：{path.relative_to(root)}")
    dependencies = json.loads(args.dependency_index.read_text(encoding="utf-8-sig"))
    metadata = json.loads(args.cargo_metadata.read_text(encoding="utf-8-sig"))
    lock = tomllib.loads((root / "Cargo.lock").read_text(encoding="utf-8"))
    checksums = {(p["name"], p["version"]): p.get("checksum") for p in lock["package"]}
    archives = []
    snapshots = []
    for package in dependencies:
        name = f"{package['name']}-{package['version']}.crate"
        info = next(p for p in metadata["packages"] if p["name"] == package["name"] and p["version"] == package["version"])
        if (info.get("source") or "").startswith("git+"):
            snapshots.append((Path(info["manifest_path"]).parent, name.removesuffix(".crate"), info["source"]))
            continue
        matches = list(args.cargo_cache.glob(f"*/{name}"))
        if len(matches) != 1:
            raise ValueError(f"缺少或存在歧义的锁定依赖源码：{name}")
        with matches[0].open("rb") as stream:
            assert hashlib.file_digest(stream, "sha256").hexdigest() == checksums[(package["name"], package["version"])], name
        archives.append(matches[0])
    args.output.parent.mkdir(parents=True, exist_ok=True)
    prefix = "vakyu-source/"
    with zipfile.ZipFile(args.output, "w", zipfile.ZIP_DEFLATED, compresslevel=6) as archive:
        for path in sorted(files):
            archive.write(path, prefix + path.relative_to(root).as_posix())
        for path in archives:
            archive.write(path, prefix + "dependency-sources/" + path.name)
        for directory, name, source in snapshots:
            for path in directory.rglob("*"):
                if path.is_file() and not path.is_symlink() and not any(p in {".git", "target", "__pycache__"} for p in path.relative_to(directory).parts):
                    archive.write(path, prefix + "dependency-sources/" + name + "/" + path.relative_to(directory).as_posix())
            archive.writestr(prefix + "dependency-sources/" + name + "/SOURCE-REVISION.txt", source + "\n")
        archive.write(args.dependency_index, prefix + "dependency-sources/index.json")
        archive.writestr(prefix + "SOURCE-README.txt", """Vakyu 对应修改源码（GPL-3.0-or-later，详见 LICENSE）

此归档对应随附 Windows 安装包的工作树，包含完整应用源码、资源、构建与安装脚本，
以及 Windows normal/build 依赖的原始 .crate 源码归档（tar.gz 格式）；Git 依赖另附锁定版本源码快照。
没有 .git、目标程序、中间文件、个人配置、API 密钥、用户词库或录音。

构建指南：apps/windows/installer/README.md。
在 Windows x64 安装 MSVC Build Tools、Rust、i686/x64 MSVC 标准库和 Inno Setup。
构建本次 x64 使用 Rust 1.99.0；x86 使用仓库锁定的 Rust 1.96.0。
Cargo.lock 固定依赖版本，dependency-sources/index.json 包含来源和许可证。
语音下载地址和 SHA256 在 voice-sources.json；prepare-runtime.ps1 可重建离线组件。
完整模型与微软运行库同时包含在安装包中，其独立许可证位于安装目录 licenses。
词库生成方法、资源来源见 tools、assets 各目录及 docs/notes/crate-notes.md。

向他人提供二进制时，请同时提供本源码归档与相应许可，保留原项目版权声明。
""")
    digest = hashlib.file_digest(args.output.open("rb"), "sha256").hexdigest()
    args.output.with_suffix(args.output.suffix + ".sha256").write_text(digest + "  " + args.output.name + "\n", encoding="ascii")
    print(json.dumps({"source_files": len(files), "dependency_archives": len(archives), "git_dependency_snapshots": len(snapshots), "bytes": args.output.stat().st_size, "sha256": digest}, indent=2))


if __name__ == "__main__":
    main()
