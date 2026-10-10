"""把真实麦克风振幅转为音量；独立线程只发送最新值，不阻塞采集回调。"""

import math
import threading

import numpy as np


def audio_level(samples):
    values = np.asarray(samples, dtype=np.float32)
    if values.size == 0:
        return 0
    values = np.nan_to_num(values, nan=0.0, posinf=0.0, neginf=0.0)
    values = np.clip(values, -1.0, 1.0)
    rms = math.sqrt(float(np.mean(values * values)))
    peak = float(np.max(np.abs(values)))
    amplitude = max(rms, peak * 0.25)
    # -52 dB 以下归零；对数映射让日常轻声可见，避免环境底噪持续跳动。
    if amplitude <= 0.0025:
        return 0
    return min(100, max(0, round(50.0 * math.log10(amplitude / 0.0025))))


class LevelMeter:
    def __init__(self, emit, requested, threaded=True):
        self.emit = emit
        self.requested = requested
        self.state = (0, None)
        self.pending = None
        self.last = None
        self.alive = True
        self.wake = threading.Event()
        if threaded:
            threading.Thread(target=self._run, name="voice-meter", daemon=True).start()

    def begin(self, generation):
        self.state = (self.state[0] + 1, generation)
        self.pending = None
        self.last = None
        self.wake.set()

    def offer(self, generation, samples):
        state = self.state
        if not self.alive or state[1] != generation:
            return
        level = audio_level(samples)
        if self.state is state:
            self.pending = (state, generation, level, False)

    def clear(self):
        old = self.state
        self.state = (old[0] + 1, None)
        # 只在实际报过音量时发送归零，未开麦/无回调时不产生假波形。
        had_level = self.last is not None and self.last[0] == old and self.last[2] != 0
        self.pending = (self.state, old[1], 0, True) if had_level else None
        self.last = None
        self.wake.set()

    def close(self):
        self.alive = False
        self.state = (self.state[0] + 1, None)
        self.pending = None
        self.wake.set()

    def publish(self):
        item = self.pending
        if not self.alive or item is None:
            return
        state, generation, level, cleared = item
        if state is not self.state or (not cleared and not self.requested(generation)):
            return
        # 只有新音频块才发送。采集回调会更新 pending；发布线程按 30Hz 合并，
        # 避免为每个很小的 PCM 块都序列化 JSON 并写管道。
        if item is self.last:
            return
        # 不持采集锁写管道；父进程同样按代次与录音状态丢弃取消瞬间的旧事件。
        self.emit("level", generation, level=level)
        if state is self.state:
            self.last = item

    def _run(self):
        while self.alive:
            # 未录音时阻塞等开始/停止通知，不让闲置音量计持续唤醒 CPU。
            timeout = 1.0 / 30.0 if self.state[1] is not None else None
            self.wake.wait(timeout)
            self.wake.clear()
            try:
                self.publish()
            except Exception:
                # 管道已关闭时由主控制生命周期退出，不让音量计影响停止录音。
                self.close()
