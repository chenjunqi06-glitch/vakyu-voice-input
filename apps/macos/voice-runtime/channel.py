"""控制线程先更新代次，旧识别即使尚在计算也无法提交；音频队列有上限。"""

import json
import queue
import threading

from pcm import MAX_ENCODED_BYTES, decode_audio


class InputChannel:
    def __init__(self, emit):
        self.emit = emit
        self.events = queue.Queue(maxsize=256)
        self.lock = threading.Lock()
        self.generation = None
        self.accepting = False
        self.alive = True

    def current(self, generation):
        with self.lock:
            return self.alive and generation == self.generation

    def invalidate(self, generation):
        with self.lock:
            if generation != self.generation:
                return
            self.generation = None
            self.accepting = False

    def _clear(self):
        while True:
            try:
                self.events.get_nowait()
            except queue.Empty:
                return

    def submit(self, request):
        command = request.get("command")
        if command == "shutdown":
            with self.lock:
                self.alive, self.accepting, self.generation = False, False, None
            self._clear()
            self.events.put_nowait(("shutdown", None, None))
            return False
        generation = request.get("generation")
        if type(generation) is not int or not 0 < generation < 2**64:
            raise ValueError("语音会话编号无效")
        if command == "start":
            language = request.get("language", "zh")
            hint = request.get("practice_text")
            if language not in ("zh", "en"):
                raise ValueError("不支持的识别语言")
            if hint is not None and not isinstance(hint, str):
                raise ValueError("跟读提示格式无效")
            with self.lock:
                self.generation, self.accepting = generation, True
            self._clear()
            self.events.put_nowait((command, generation, ((hint or "")[:8192], language)))
        elif command == "cancel":
            if self.current(generation):
                self.invalidate(generation)
                self._clear()
                self.events.put_nowait((command, generation, None))
        elif command in ("audio", "stop"):
            with self.lock:
                accepting = self.alive and self.accepting and self.generation == generation
                if accepting and command == "stop":
                    self.accepting = False
            if not accepting:
                return True
            try:
                payload = decode_audio(request) if command == "audio" else None
                self.events.put_nowait((command, generation, payload))
            except (ValueError, queue.Full):
                self.invalidate(generation)
                self._clear()
                self.events.put_nowait(("cancel", generation, None))
                self.emit("error", generation, message="音频输入异常或处理积压，请重新录音")
        else:
            raise ValueError("不支持的语音命令")
        return True

    def read(self, source):
        try:
            while self.alive:
                line = source.readline(MAX_ENCODED_BYTES + 2048)
                if not line:
                    break
                if not line.endswith("\n") and len(line) >= MAX_ENCODED_BYTES + 2048:
                    self.emit("error", message="语音消息过长，已停止接收")
                    break
                try:
                    request = json.loads(line)
                    if not isinstance(request, dict):
                        raise ValueError("无效消息")
                    if not self.submit(request):
                        return
                except (ValueError, TypeError):
                    self.emit("error", message="忽略无效语音控制消息")
        finally:
            self.submit({"command": "shutdown"})
