"""验证原生采集的 Float32LE 单声道数据；只接受有界、有限的 PCM。"""

import base64
import binascii

import numpy as np

MAX_ENCODED_BYTES = 256 * 1024


def decode_audio(request):
    rate = request.get("sample_rate")
    encoded = request.get("pcm")
    if type(rate) is not int or not 8000 <= rate <= 48000:
        raise ValueError("录音采样率无效")
    if not isinstance(encoded, str) or not 0 < len(encoded) <= MAX_ENCODED_BYTES:
        raise ValueError("录音数据包大小无效")
    try:
        raw = base64.b64decode(encoded, validate=True)
    except (ValueError, binascii.Error) as error:
        raise ValueError("录音数据编码无效") from error
    if not raw or len(raw) % 4:
        raise ValueError("录音数据没有对齐 Float32")
    samples = np.frombuffer(raw, dtype="<f4")
    if samples.size > rate or not np.isfinite(samples).all():
        raise ValueError("录音数据包过长或包含无效音量")
    return rate, np.clip(samples, -1.0, 1.0)
