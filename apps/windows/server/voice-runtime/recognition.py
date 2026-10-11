"""共享中英流式识别；断句保留原文，松键只排空已有音频。"""

import time
import wave
from pathlib import Path

import numpy as np
import sherpa_onnx

from bpe_vocab import prepare_bpe_vocab
from dictation_context import dictation_hotwords
from practice_context import practice_hotwords


def chinese_context_tokens(model):
    """只提示模型有实际 ID 的汉字；未知字不能让整组热词编码失败。"""
    try:
        pieces = (line.rsplit(" ", 1)[0] for line in
                  (model / "tokens.txt").read_text(encoding="utf-8").splitlines())
        return frozenset(piece[1] for piece in pieces if len(piece) == 2
                         and piece[0] == "▁" and
                         ("\u3400" <= piece[1] <= "\u4dbf" or "\u4e00" <= piece[1] <= "\u9fff"))
    except (OSError, UnicodeError):
        return frozenset()


def join_segments(previous, current):
    if (previous and current and previous[-1].isascii() and previous[-1].isalnum()
            and current[0].isascii() and current[0].isalnum()):
        return previous + " " + current
    return previous + current


def create_recognizer(models: Path):
    model = models / "streaming"
    options = dict(
        tokens=str(model / "tokens.txt"),
        encoder=str(next(model.glob("encoder*.onnx"))),
        decoder=str(next(model.glob("decoder*.onnx"))),
        joiner=str(next(model.glob("joiner*.onnx"))),
        num_threads=2,
        sample_rate=16000,
        enable_endpoint_detection=True,
        # English practice must survive natural pauses between words and clauses.
        # Keep endpoint detection enabled for long recordings, but wait longer
        # before resetting the stream so a short mid-sentence pause is not lost.
        rule1_min_trailing_silence=3.2,
        rule2_min_trailing_silence=1.5,
        rule3_min_utterance_length=25.0,
        provider="cpu",
    )
    vocab = prepare_bpe_vocab(model)
    if vocab is not None:
        try:
            recognizer = sherpa_onnx.OnlineRecognizer.from_transducer(
                **options, decoding_method="modified_beam_search", max_active_paths=4,
                hotwords_score=1.0, modeling_unit="bpe", bpe_vocab=str(vocab),
            )
            recognizer.practice_context_enabled = True
            recognizer.practice_chinese_tokens = chinese_context_tokens(model)
            return recognizer
        except Exception:
            pass
    # 词表缺失、只读或提示初始化失败都不能阻止常规录音。
    recognizer = sherpa_onnx.OnlineRecognizer.from_transducer(
        **options, decoding_method="greedy_search")
    recognizer.practice_context_enabled = False
    return recognizer


class Recognition:
    def __init__(self, recognizer, practice_text=None, language=None, dictation_vocabulary=None):
        self.recognizer = recognizer
        self.language = language or ("en" if practice_text else "zh")
        if self.language not in ("zh", "en"):
            raise ValueError("unsupported recognition language")
        chinese_tokens = getattr(recognizer, "practice_chinese_tokens", None)
        if not isinstance(chinese_tokens, frozenset):
            chinese_tokens = None
        hints = practice_hotwords(practice_text, self.language, chinese_tokens)
        if not practice_text:
            hints = dictation_hotwords(dictation_vocabulary)
        if hints and getattr(recognizer, "practice_context_enabled", False) is True:
            try:
                self.stream = recognizer.create_stream(hints)
            except Exception:
                self.stream = recognizer.create_stream()
        else:
            self.stream = recognizer.create_stream()
        self.completed = ""
        self.partial = ""
        self.latest = ""
        self.sample_rate = None

    def accept(self, rate, samples):
        if self.sample_rate is not None and self.sample_rate != rate:
            raise ValueError("录音期间采样率发生变化")
        self.sample_rate = rate
        self.stream.accept_waveform(rate, samples)
        self._decode()
        current = self.recognizer.get_result(self.stream)
        self._update(current)
        if self.recognizer.is_endpoint(self.stream):
            self.completed = self.latest
            self.partial = ""
            self.recognizer.reset(self.stream)
        return self.latest

    def finish(self):
        # 流式网络有固定右上下文；补静音不会再采集麦克风。
        rate = self.sample_rate or 16000
        self.stream.accept_waveform(rate, np.zeros(rate, dtype=np.float32))
        self.stream.input_finished()
        self._decode()
        self._update(self.recognizer.get_result(self.stream))
        return self.latest

    def _update(self, current):
        # 模型每约 480ms 解码一次；其它音频块与长停顿不重复复制整段口述。
        if current != self.partial:
            self.latest = join_segments(self.completed, current)
            self.partial = current

    def _decode(self):
        while self.recognizer.is_ready(self.stream):
            self.recognizer.decode_stream(self.stream)


def recognize_wave(models, filename):
    before = time.perf_counter()
    recognizer = create_recognizer(models)
    initialized = time.perf_counter()
    with wave.open(str(filename), "rb") as audio:
        if audio.getsampwidth() != 2:
            raise ValueError("回放文件必须使用 16 位 PCM")
        rate = audio.getframerate()
        samples = np.frombuffer(audio.readframes(audio.getnframes()), dtype=np.int16)
        channels = audio.getnchannels()
        samples = samples.astype(np.float32).reshape(-1, channels).mean(axis=1) / 32768
    session = Recognition(recognizer)
    steps = []
    block = int(rate * 0.05)
    previous = ""
    for offset in range(0, len(samples), block):
        text = session.accept(rate, samples[offset:offset + block])
        if text != previous:
            steps.append({"audio_seconds": round((offset + block) / rate, 3), "text": text})
            previous = text
    before_finish = time.perf_counter()
    text = session.finish()
    end = time.perf_counter()
    return {
        "text": text,
        "duration_seconds": len(samples) / rate,
        "load_seconds": initialized - before,
        "decode_seconds": end - initialized,
        "finish_seconds": end - before_finish,
        "interim": steps,
    }
