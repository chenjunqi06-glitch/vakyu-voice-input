"""为已校验的 X-ASR 词表生成热词分词配置；不含用户语句。"""

import hashlib
import os


XASR_TOKENS_SHA256 = "b818a60878b9aae978cbb8ad594acbd403d76d1af2e31ef4197c84e2dbdba27c"


def prepare_bpe_vocab(model):
    temporary = None
    try:
        tokens = (model / "tokens.txt").read_bytes()
        # 此配方依赖该模型的 BPE 顺序，不能套用到未知模型。
        if hashlib.sha256(tokens).hexdigest() != XASR_TOKENS_SHA256:
            return None
        rows, have, bare = [], set(), set()
        for line in tokens.decode("utf-8").splitlines():
            piece, index = line.rsplit(" ", 1)
            rows.append(f"{piece}\t{-int(index)}")
            have.add(piece)
            if len(piece) == 2 and piece[0] == "▁" and is_cjk(piece[1]):
                bare.add(piece[1])
        # 与 X-ASR make-bpe-vocab.ps1 相同：低分裸字供 BPE 合并引导。
        if "▁" not in have:
            rows.append("▁\t-1")
        rows.extend(f"{char}\t-999999" for char in sorted(bare - have))
        content = ("\n".join(rows) + "\n").encode("utf-8")
        target = model / "bpe.vocab"
        if target.exists() and target.read_bytes() == content:
            return target
        temporary = model / f".bpe-vocab-{os.getpid()}.tmp"
        temporary.write_bytes(content)
        temporary.replace(target)
        return target
    except (OSError, UnicodeError, ValueError):
        return None
    finally:
        if temporary is not None:
            try:
                temporary.unlink(missing_ok=True)
            except OSError:
                pass


def is_cjk(char):
    return ("\u3400" <= char <= "\u4dbf" or "\u4e00" <= char <= "\u9fff"
            or "\uf900" <= char <= "\ufaff")
