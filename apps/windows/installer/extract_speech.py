"""从已校验哈希的 sherpa-onnx Piper 归档里取出自然人声需要的文件，放进 models/speech/en/。

只取模型、tokens、模型说明和 espeak-ng 音标数据；espeak-ng 的词典只留英语（en_dict），其余语言词典约 16MB 用不上。
用法：python extract_speech.py <归档.tar.bz2> <归档内根目录名> <输出目录>
"""

import shutil
import sys
import tarfile
from pathlib import Path, PurePosixPath


def wanted(relative):
    """归档内相对路径 → 输出相对路径；不要的返回 None。"""
    parts = PurePosixPath(relative).parts
    if not parts or any(part in ("", ".", "..") for part in parts):
        return None
    name = parts[-1]
    if len(parts) == 1:
        if name.endswith(".onnx"):
            return "model.onnx"
        if name in ("tokens.txt", "MODEL_CARD"):
            return name
        return None
    if parts[0] != "espeak-ng-data":
        return None
    if len(parts) == 2 and name.endswith("_dict") and name != "en_dict":
        return None
    return "/".join(parts)


def extract(archive, root, output):
    output = Path(output)
    prefix = root.rstrip("/") + "/"
    copied = 0
    with tarfile.open(archive, "r:bz2") as tar:
        for member in tar:
            if not member.isfile() or not member.name.startswith(prefix):
                continue
            target = wanted(member.name[len(prefix):])
            if target is None:
                continue
            path = output.joinpath(*target.split("/"))
            path.parent.mkdir(parents=True, exist_ok=True)
            source = tar.extractfile(member)
            with source, open(path, "wb") as out:
                shutil.copyfileobj(source, out)
            copied += 1
    if copied == 0:
        raise SystemExit("归档里没有找到自然人声文件")
    return copied


if __name__ == "__main__":
    if len(sys.argv) != 4:
        raise SystemExit(__doc__)
    print(extract(sys.argv[1], sys.argv[2], sys.argv[3]))
