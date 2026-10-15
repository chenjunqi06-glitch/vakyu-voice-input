"""本机自然人声：sherpa-onnx 运行 Piper（VITS）英语模型，只在本机 CPU 合成，不联网、不保存音频。

模型目录 `models/speech/en/` 由打包脚本放入：`model.onnx`、`tokens.txt` 与 `espeak-ng-data/`（音标规则）。
随包模型是 LibriTTS-R 多说话人模型，用 `speaker` 选音色；合成一句英语在普通 CPU 上约零点几秒。
"""

from pathlib import Path

REQUIRED = ("model.onnx", "tokens.txt", "espeak-ng-data/phontab", "espeak-ng-data/en_dict")

# 句子很长时一次只合成到这里，避免老电脑卡住几秒。
MAX_TEXT_CHARS = 600

MIN_SPEED = 0.5

MAX_SPEED = 2.0


class SpeechError(Exception):
    """给用户看的失败原因。"""


def model_files_present(root):
    root = Path(root)
    return all((root / name).is_file() for name in REQUIRED)


def clamp_speed(speed):
    try:
        value = float(speed)
    except (TypeError, ValueError):
        return 1.0
    if value != value:  # NaN
        return 1.0
    return min(MAX_SPEED, max(MIN_SPEED, value))


def clean_text(text):
    """去掉控制字符、压掉多余空白；只朗读前 MAX_TEXT_CHARS 个字符。"""
    if not isinstance(text, str):
        return ""
    printable = "".join(ch if ch.isprintable() else " " for ch in text)
    return " ".join(printable.split())[:MAX_TEXT_CHARS]


def sherpa_engine(root, threads):
    import sherpa_onnx

    root = Path(root)
    config = sherpa_onnx.OfflineTtsConfig(
        model=sherpa_onnx.OfflineTtsModelConfig(
            vits=sherpa_onnx.OfflineTtsVitsModelConfig(
                model=str(root / "model.onnx"),
                lexicon="",
                data_dir=str(root / "espeak-ng-data"),
                tokens=str(root / "tokens.txt"),
            ),
            provider="cpu",
            num_threads=threads,
            debug=False,
        ),
        max_num_sentences=1,
    )
    validate = getattr(config, "validate", None)
    if validate is not None and not validate():
        raise SpeechError("自然人声模型文件不完整，请重新安装 Vakyu 完整安装包")
    return sherpa_onnx.OfflineTts(config)


class NaturalVoice:
    """加载一次模型，之后每句只做合成；返回 float32 单声道采样与采样率。"""

    def __init__(self, root, engine_factory=sherpa_engine, threads=None):
        if not model_files_present(root):
            raise SpeechError("自然人声模型没有安装完整，请重新安装 Vakyu 完整安装包")
        if threads is None:
            import os

            threads = max(1, min(2, (os.cpu_count() or 2) // 2))
        self.engine = engine_factory(root, threads)
        speakers = getattr(self.engine, "num_speakers", 1)
        self.speakers = speakers if isinstance(speakers, int) and speakers > 0 else 1
        self._cached = None

    def warmup(self):
        """提前初始化推理图；固定短句仅在内存里合成，不播放。"""
        self.synthesize("Hello.", 0, 1.0)

    def speaker(self, requested):
        if isinstance(requested, int) and 0 <= requested < self.speakers:
            return requested
        return 0

    def synthesize(self, text, speaker=0, speed=1.0):
        text = clean_text(text)
        if not text:
            raise SpeechError("没有可朗读的句子")
        key = (text, self.speaker(speaker), clamp_speed(speed))
        if self._cached is not None and self._cached[0] == key:
            return self._cached[1]
        audio = self.engine.generate(text, sid=key[1], speed=key[2])
        samples = getattr(audio, "samples", None)
        rate = getattr(audio, "sample_rate", 0)
        if samples is None or len(samples) == 0 or not rate:
            raise SpeechError("自然人声合成失败，可继续跟读")
        result = (samples, int(rate))
        # 只留上一句，重听时复用；音色或语速变化都会使缓存失效。
        self._cached = (key, result)
        return result
