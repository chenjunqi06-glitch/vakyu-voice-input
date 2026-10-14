"""本机离线中英翻译：CTranslate2 + OPUS-MT（Helsinki-NLP），只在本机 CPU 运行，不联网、不记录正文。

模型目录 `models/translate/<方向>/` 由打包脚本从官方 Transformers 权重转换而来，包含 `model.bin`、
`shared_vocabulary.json`（或 `source_vocabulary.json`/`target_vocabulary.json`）、`source.spm`、`target.spm`。
英译中模型一次覆盖多种中文变体，必须在句首加目标标记 `>>cmn_Hans<<` 才输出简体。
"""

import os
import re
from pathlib import Path

DIRECTIONS = {
    "en": ("zh-en", None),
    "zh": ("en-zh", ">>cmn_Hans<<"),
}

# 单句过长时模型容易漏译，按逗号再切一次。
MAX_SENTENCE_CHARS = 220

_ZH_SENTENCE = re.compile(r"[^。！？；!?;…]+[。！？；!?;…]*")
_EN_SENTENCE = re.compile(r"[^.!?;]+(?:[.!?;]+[\"')\]]*|$)")
_ZH_CLAUSE = re.compile(r"[^，、,]+[，、,]*")
_EN_CLAUSE = re.compile(r"[^,]+,*")


class TranslationError(Exception):
    """给用户看的失败原因。"""


def model_files_present(root):
    """两个方向的模型文件都在；只做存在检查，不加载模型。"""
    for folder, _ in DIRECTIONS.values():
        path = Path(root) / folder
        if not (path / "model.bin").is_file():
            return False
        if not ((path / "source.spm").is_file() and (path / "target.spm").is_file()):
            return False
    return True


def split_sentences(line, source):
    """按句切分一行；长句再按逗号切，保证每段都不空。"""
    pattern = _ZH_SENTENCE if source == "zh" else _EN_SENTENCE
    clause = _ZH_CLAUSE if source == "zh" else _EN_CLAUSE
    pieces = []
    for match in pattern.finditer(line):
        sentence = match.group(0).strip()
        if not sentence:
            continue
        if len(sentence) <= MAX_SENTENCE_CHARS:
            pieces.append(sentence)
            continue
        pieces.extend(part.strip() for part in clause.findall(sentence) if part.strip())
    return pieces or ([line.strip()] if line.strip() else [])


def join_sentences(sentences, target):
    if target == "en":
        return " ".join(sentence.strip() for sentence in sentences if sentence.strip())
    return "".join(sentence.strip() for sentence in sentences if sentence.strip())


class OfflineTranslator:
    """按方向懒加载模型；`factory(path, prefix)` 返回带 `translate(list[str]) -> list[str]` 的对象，测试可替换。"""

    def __init__(self, root, factory=None):
        self.root = Path(root)
        self.factory = factory or Ct2Model
        self.models = {}

    def translate(self, text, target):
        if target not in DIRECTIONS:
            raise TranslationError("离线翻译只支持中文和英文")
        model = self._model(target)
        source = "zh" if target == "en" else "en"
        lines = []
        for line in str(text).replace("\r\n", "\n").replace("\r", "\n").split("\n"):
            sentences = split_sentences(line, source)
            if not sentences:
                lines.append("")
                continue
            lines.append(join_sentences(model.translate(sentences), target))
        result = "\n".join(lines).strip("\n")
        if not result.strip():
            raise TranslationError("离线翻译没有得到译文，请换个说法重试")
        return result

    def _model(self, target):
        model = self.models.get(target)
        if model is None:
            folder, prefix = DIRECTIONS[target]
            path = self.root / folder
            if not (path / "model.bin").is_file():
                raise TranslationError("离线翻译模型没有安装完整，请重新安装 Vakyu 完整安装包")
            try:
                model = self.factory(path, prefix)
            except TranslationError:
                raise
            except Exception as error:  # 原生库加载失败只给可读提示，不输出正文。
                raise TranslationError(f"离线翻译模型加载失败：{type(error).__name__}") from error
            self.models[target] = model
        return model


class Ct2Model:
    """一个方向的 CTranslate2 模型；int8 CPU 推理，最多用 4 个线程。"""

    def __init__(self, path, prefix):
        import ctranslate2
        import sentencepiece

        threads = max(1, min(4, (os.cpu_count() or 2) // 2))
        self.translator = ctranslate2.Translator(
            str(path), device="cpu", compute_type="int8", inter_threads=1, intra_threads=threads
        )
        self.source = sentencepiece.SentencePieceProcessor(model_file=str(path / "source.spm"))
        self.target = sentencepiece.SentencePieceProcessor(model_file=str(path / "target.spm"))
        self.prefix = prefix

    def translate(self, sentences):
        batch = []
        for sentence in sentences:
            tokens = self.source.encode(sentence, out_type=str)
            if self.prefix:
                tokens = [self.prefix] + tokens
            batch.append(tokens + ["</s>"])
        results = self.translator.translate_batch(
            batch, beam_size=2, max_decoding_length=256, max_batch_size=16
        )
        return [self.target.decode(result.hypotheses[0]) for result in results]
