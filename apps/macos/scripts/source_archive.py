"""从 Git 清单与明确的新增源码范围制作源码交付包，不归档本机产物或用户资料。"""

import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import stat
import subprocess
import zipfile

from distribution_policy import check_credentials, private_path

ROOT = Path(__file__).resolve().parents[3]
ARCHIVE_ROOT = "vakyu-macos-source"
SOURCE_ROOTS = frozenset({"apps", "crates", "assets", "docs", "tools", ".github", ".cargo", ".githooks"})
ROOT_FILES = frozenset({"Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "LICENSE", "README.md",
                        "AGENTS.md", "CLAUDE.md", "CHANGELOG.md", "SECURITY.md", "UPSTREAM.md", ".gitignore", ".gitattributes"})
# 这些目录包含本轮及此前尚未提交、但当前 workspace 引用的源码。范围不会延伸到 target 或用户目录。
EXTRA_PATHS = (
    ".github/workflows/macos-port.yml", "apps/macos", "apps/voice-service",
    "apps/windows/server/src", "apps/windows/server/voice-runtime", "apps/windows/settings/src",
    "apps/windows/tsf/src", "apps/windows/tsf/resources", "apps/windows/scripts", "apps/windows/installer",
    "apps/linux/fcitx5/src", "crates", "assets/icon", "assets/pronunciation", "assets/lexicon", "docs",
)
SOURCE_SUFFIXES = frozenset({".rs", ".swift", ".py", ".sh", ".ps1", ".toml", ".lock", ".md",
                            ".txt", ".tsv", ".dict", ".json", ".yaml", ".yml", ".xml", ".html",
                            ".plist", ".strings", ".entitlements", ".conf", ".iss", ".cpp", ".c",
                            ".h", ".hpp", ".m", ".mm", ".png", ".ico", ".svg", ".pdf", ".alpha"})
REQUIRED = (
    "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "apps/macos/Cargo.toml",
    "apps/macos/companion/Package.swift", "apps/macos/scripts/bundle.sh",
    "apps/macos/voice-runtime/external_main.py", "apps/voice-service/Cargo.toml",
    "crates/qingjian-practice/Cargo.toml", "apps/windows/server/voice-runtime/recognition.py",
    "assets/icon/windows/vakyu-icon.png", "assets/icon/windows/vakyu-logo.png", "tools/release/data.lock",
    "assets/glossary/glossary-zh.tsv", "assets/glossary/glossary-en.tsv", "assets/pronunciation/cmudict.dict",
    "apps/macos/scripts/distribution_policy.py", "apps/macos/scripts/check_port.py",
)


def git_paths(arguments):
    output = subprocess.check_output(["git", "-C", str(ROOT), "ls-files", "-z", *arguments])
    return {value for value in output.decode("utf-8").split("\0") if value}


def permitted(relative):
    path = PurePosixPath(relative)
    if path.is_absolute() or ".." in path.parts or private_path(relative, source=True):
        return False
    if len(path.parts) == 1:
        return relative in ROOT_FILES
    if path.parts[0] not in SOURCE_ROOTS:
        return False
    if any(part in {"bin", "obj", "dependencies", "vendor", "dist", ".swiftpm"} for part in path.parts[1:-1]):
        return False
    return (path.suffix.lower() in SOURCE_SUFFIXES or path.name in
            {"CMakeLists.txt", "LICENSE", "NOTICE", ".gitignore", ".gitattributes", "preinstall",
             "postinstall", "pre-commit", "commit-msg", "pre-push"})


def selected_paths():
    if not (ROOT / ".git").exists():
        raise ValueError("制作源码包需要原始仓库的 Git 清单；解压后的源码可直接构建，无需再次归档")
    tracked = git_paths(["--cached", "--", *sorted(SOURCE_ROOTS), *sorted(ROOT_FILES)])
    additional = git_paths(["--others", "--exclude-standard", "--", *EXTRA_PATHS])
    selected = []
    for relative in sorted(tracked | additional):
        if not permitted(relative):
            continue
        path = ROOT / relative
        if not path.is_file():
            continue
        if path.is_symlink() or not path.resolve().is_relative_to(ROOT.resolve()):
            raise ValueError(f"源码包不跟随链接：{relative}")
        selected.append(relative)
    missing = sorted(set(REQUIRED) - set(selected))
    if missing:
        raise ValueError(f"缺少必要源码：{', '.join(missing)}")
    return selected


def content_for(relative):
    content = (ROOT / relative).read_bytes()
    check_credentials(content, relative)
    if relative.endswith(".sh") or relative.startswith("apps/macos/pkg/scripts/") or relative.startswith(".githooks/"):
        content = content.replace(b"\r\n", b"\n")
        if b"\r" in content:
            raise ValueError(f"shell 文件含不完整换行：{relative}")
    return content


def metadata(relative, content):
    executable = content.startswith(b"#!") or relative.startswith("apps/macos/pkg/scripts/")
    return {"path": relative, "bytes": len(content), "sha256": hashlib.sha256(content).hexdigest(),
            "mode": "0755" if executable else "0644"}


def write_member(archive, relative, content, mode):
    entry = zipfile.ZipInfo(f"{ARCHIVE_ROOT}/{relative}", date_time=(2026, 10, 9, 0, 0, 0))
    entry.create_system = 3
    entry.external_attr = (stat.S_IFREG | int(mode, 8)) << 16
    entry.compress_type = zipfile.ZIP_DEFLATED
    archive.writestr(entry, content)


def create(output, list_only=False):
    output = output.resolve()
    if output.is_relative_to(ROOT.resolve()):
        raise ValueError("源码归档必须输出到仓库外，避免自身进入交付")
    selected = selected_paths()
    files = []
    archive = None
    partial = output.with_suffix(output.suffix + ".partial")
    output.parent.mkdir(parents=True, exist_ok=True)
    try:
        if not list_only:
            archive = zipfile.ZipFile(partial, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=6)
        for relative in selected:
            content = content_for(relative)
            entry = metadata(relative, content)
            files.append(entry)
            if archive:
                write_member(archive, relative, content, entry["mode"])
        manifest = {"format": 1, "archive_root": ARCHIVE_ROOT, "file_count": len(files),
                    "total_uncompressed_bytes": sum(item["bytes"] for item in files),
                    "files": files}
        encoded = (json.dumps(manifest, ensure_ascii=False, indent=2) + "\n").encode("utf-8")
        if archive:
            write_member(archive, "SOURCE_MANIFEST.json", encoded, "0644")
            archive.close()
            archive = None
            partial.replace(output)
            checksum = hashlib.sha256(output.read_bytes()).hexdigest()
            output.with_suffix(output.suffix + ".sha256").write_text(
                f"{checksum}  {output.name}\n", encoding="utf-8")
        manifest_path = output.with_suffix(".manifest.json")
        manifest_path.write_bytes(encoded)
        print(json.dumps({"created_zip": not list_only, "archive": str(output) if not list_only else None,
                          "manifest": str(manifest_path), "files": len(files),
                          "uncompressed_bytes": manifest["total_uncompressed_bytes"]}, ensure_ascii=False))
    finally:
        if archive:
            archive.close()
        partial.unlink(missing_ok=True)


def verify(archive_path):
    with zipfile.ZipFile(archive_path) as archive:
        manifest = json.loads(archive.read(f"{ARCHIVE_ROOT}/SOURCE_MANIFEST.json"))
        expected = {f"{ARCHIVE_ROOT}/{item['path']}": item for item in manifest["files"]}
        actual = set(archive.namelist()) - {f"{ARCHIVE_ROOT}/SOURCE_MANIFEST.json"}
        assert actual == set(expected), "源码包内容与清单不一致"
        for name, item in expected.items():
            assert permitted(item["path"]), f"清单包含被排除路径：{item['path']}"
            content = archive.read(name)
            assert len(content) == item["bytes"] and hashlib.sha256(content).hexdigest() == item["sha256"], name
            check_credentials(content, item["path"])
        print(f"源码包验证通过：{len(expected)} 个文件")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=ROOT.parent / "outputs/vakyu-macos-arm64-source.zip")
    parser.add_argument("--list-only", action="store_true", help="只生成带 SHA256 的清单，不创建 zip")
    parser.add_argument("--verify", type=Path, help="核验已有 zip 的文件清单、SHA256 和排除规则")
    args = parser.parse_args()
    if args.verify:
        verify(args.verify)
    else:
        create(args.output, args.list_only)
