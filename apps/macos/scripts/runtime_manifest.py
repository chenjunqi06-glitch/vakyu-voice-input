"""运行时文件清单；Mach-O 签名后重算，再由外层应用签名封存。"""

import hashlib
import json
from pathlib import Path
import sys

MANIFEST = "SHA256SUMS.json"


def inventory(root):
    entries = {}
    for path in sorted(root.rglob("*")):
        if not path.is_file() or path.is_symlink() or path == root / MANIFEST:
            continue
        digest = hashlib.sha256()
        with path.open("rb") as source:
            for chunk in iter(lambda: source.read(1024 * 1024), b""):
                digest.update(chunk)
        entries[path.relative_to(root).as_posix()] = digest.hexdigest()
    return entries


def write_manifest(root):
    (root / MANIFEST).write_text(json.dumps(inventory(root), indent=2) + "\n", encoding="utf-8")


def verify_manifest(root):
    expected = json.loads((root / MANIFEST).read_text(encoding="utf-8"))
    actual = inventory(root)
    if expected != actual:
        changed = sorted(key for key in expected.keys() | actual.keys()
                         if expected.get(key) != actual.get(key))
        raise ValueError(f"运行时文件校验失败：{', '.join(changed[:5])}")


if __name__ == "__main__":
    write_manifest(Path(sys.argv[1]))
