"""控制线程持有麦克风；停止和取消不等待识别线程完成推理。"""

import os
import threading

# 分发包使用标准 Windows 音频后端，不让其他音频软件的环境变量切到未打包的 ASIO。
os.environ.pop("SD_ENABLE_ASIO", None)
import sounddevice as sd

from audio_meter import LevelMeter


def list_devices():
    devices = sd.query_devices()
    hosts = sd.query_hostapis()
    default = int(sd.default.device[0])
    inputs = [
        {"index": index, "name": device["name"],
         "hostapi": hosts[device["hostapi"]]["name"],
         "default_samplerate": device["default_samplerate"],
         "channels": device["max_input_channels"], "default": index == default}
        for index, device in enumerate(devices) if device["max_input_channels"] > 0
    ]
    return {"default_input": default, "inputs": inputs}


class Capture:
    def __init__(self, events, emit):
        self.events = events
        self.emit = emit
        self.lock = threading.RLock()
        self.generation = None
        self.held = False
        self.alive = True
        self.stream = None
        self.starting = None
        self.unclosed = None
        self.pending_release = []
        self.meter = LevelMeter(emit, self.requested)

    def command(self, command, generation, practice_text=None, language=None):
        with self.lock:
            if command == "start":
                self.meter.clear()
                self._close()
                self.generation = generation
                self.held = True
                self.meter.begin(generation)
                language = language or ("en" if practice_text else "zh")
                self.events.put(("start", generation, (practice_text, language)))
            elif command == "stop" and generation == self.generation:
                self.meter.clear()
                self.held = False
                if self._close():
                    self.emit("stopped", generation)
                else:
                    self.pending_release.append(("stopped", generation))
                self.events.put(("stop", generation, None))
            elif command == "cancel":
                self.meter.clear()
                self.held = False
                self.generation = None
                if self._close():
                    self.emit("cancelled", generation)
                else:
                    self.pending_release.append(("cancelled", generation))
                self.events.put(("cancel", generation, None))
            elif command == "shutdown":
                self.meter.close()
                self.alive = False
                self.held = False
                self.generation = None
                self._close()
                self.events.put(("shutdown", generation, None))

    def requested(self, generation):
        return (self.alive and self.held and self.generation == generation
                and self.unclosed is None)

    def cancel_if_current(self, generation):
        with self.lock:
            if self.generation == generation:
                self.command("cancel", generation)

    def open(self, generation):
        if not self.requested(generation):
            return False
        info = sd.query_devices(kind="input")
        rate = int(info["default_samplerate"])

        def callback(data, frames, timing, status):
            if not self.requested(generation):
                raise sd.CallbackAbort
            if status.input_overflow:
                self.events.put(("overflow", generation, None))
            self.meter.offer(generation, data[:, 0])
            self.events.put(("audio", generation, (rate, data[:, 0].copy())))

        # 设备初始化可能耗时；不持锁，松键仍可立即撤销开麦意图。
        with self.lock:
            if not self.requested(generation):
                return False
            self.starting = object()
        try:
            stream = sd.InputStream(samplerate=rate, channels=1, dtype="float32",
                                   blocksize=max(1, rate // 20), callback=callback)
        except Exception:
            with self.lock:
                self.starting = None
                self._release_acknowledgements()
            raise
        with self.lock:
            if not self.requested(generation):
                if self._close_stream(stream):
                    self.starting = None
                self._release_acknowledgements()
                return False
            self.stream = stream
            self.starting = stream
        try:
            # PortAudio 操作不持意图锁；也不允许 start 与 close 并发操作同一原生句柄。
            stream.start()
        except Exception:
            with self.lock:
                if self.stream is stream:
                    self.stream = None
                if self._close_stream(stream):
                    self.starting = None
                self._release_acknowledgements()
            raise
        with self.lock:
            if not self.requested(generation) or self.stream is not stream:
                if self.stream is stream:
                    self.stream = None
                if self._close_stream(stream):
                    self.starting = None
                self._release_acknowledgements()
                return False
            self.starting = None
            self.emit("started", generation)
        return True

    def _close(self):
        if self.unclosed is not None:
            return False
        if self.stream is not None:
            stream, self.stream = self.stream, None
            if self.starting is stream:
                return False
            if not self._close_stream(stream):
                return False
        return self.starting is None

    def _close_stream(self, stream):
        # 关闭失败时保留未关闭标记；不能发送成功确认，由父进程超时终止释放设备。
        self.unclosed = stream
        closed = False
        try:
            try:
                stream.abort()
            finally:
                stream.close(ignore_errors=False)
                closed = True
        except Exception:
            pass
        if closed:
            self.unclosed = None
        return closed

    def _release_acknowledgements(self):
        if self.starting is not None or self.unclosed is not None:
            return
        acknowledgements, self.pending_release = self.pending_release, []
        for event, generation in acknowledgements:
            self.emit(event, generation)
