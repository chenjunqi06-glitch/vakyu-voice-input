"""Vakyu 自然人声朗读进程入口：标准输入输出是 JSON 行，只传朗读请求与播放结果，不联网、不写日志、不存音频。

请求：{"command":"speak","id":1,"text":"…","speaker":20,"speed":0.8}、{"command":"stop"}、{"command":"shutdown"}。
回复：{"event":"ready"}；播放完 {"event":"done","id":1}；失败 {"event":"error","id":1,"message":"…"}；
模型加载失败 {"event":"fatal","message":"…"} 后退出。stop 停掉正在播放的和已收到的所有请求，被停掉的不回 done。
空闲 10 分钟自动退出释放内存，父进程下次朗读时重新启动。
"""

import argparse
import json
import queue
import sys
import threading
import time
from pathlib import Path

IDLE_SECONDS = 600

# 播放超过音频时长这么久仍没结束，按已结束处理，避免声卡异常时卡住跟读。
PLAYBACK_GRACE = 3.0

OUTPUT_LOCK = threading.Lock()


def emit(event, **fields):
    with OUTPUT_LOCK:
        print(json.dumps({"event": event, **fields}, ensure_ascii=False), flush=True)


class SoundPlayer:
    """sounddevice 默认输出设备；播放不阻塞，`wait` 轮询结束或被停。"""

    def __init__(self):
        import numpy
        import sounddevice

        self.np = numpy
        self.sd = sounddevice

    def play(self, samples, rate):
        self.sd.play(self.np.asarray(samples, dtype=self.np.float32), rate, blocking=False)

    def wait(self, seconds, cancelled):
        deadline = time.monotonic() + seconds + PLAYBACK_GRACE
        while time.monotonic() < deadline:
            if cancelled():
                return
            try:
                if not self.sd.get_stream().active:
                    return
            except RuntimeError:
                return
            time.sleep(0.02)

    def stop(self):
        try:
            self.sd.stop()
        except Exception:  # 停止失败不影响下一句。
            pass


class Session:
    """读线程与播放线程共享的取消状态；同一把锁保证「检查未取消 → 开始播放」与 stop 不交错。"""

    def __init__(self, player):
        self.player = player
        self.lock = threading.Lock()
        self.last_seen = 0
        self.cancelled_upto = 0

    def seen(self, request):
        with self.lock:
            self.last_seen = max(self.last_seen, request)

    def stop(self):
        with self.lock:
            self.cancelled_upto = self.last_seen
            self.player.stop()

    def cancelled(self, request):
        return request <= self.cancelled_upto

    def start(self, request, samples, rate):
        """未被取消就开始播放；返回是否开始。"""
        with self.lock:
            if self.cancelled(request):
                return False
            self.player.play(samples, rate)
            return True


def dispatch(line, lines, session):
    """处理读到的一行；返回 False 表示 shutdown。stop 在读线程里立即执行，不排队。"""
    try:
        command = json.loads(line)
    except ValueError:
        return True
    if not isinstance(command, dict):
        return True
    kind = command.get("command")
    if kind == "stop":
        session.stop()
    elif kind == "shutdown":
        return False
    elif kind == "speak" and isinstance(command.get("id"), int):
        session.seen(command["id"])
        lines.put(command)
    return True


def read_commands(lines, session):
    for line in sys.stdin:
        if not dispatch(line, lines, session):
            break
    # 父进程关闭管道或要求退出：立即停声，不再播放排队的句子。
    session.stop()
    lines.put(None)


def speak(voice, session, command):
    request = command["id"]
    if session.cancelled(request):
        return
    try:
        samples, rate = voice.synthesize(
            command.get("text", ""), command.get("speaker", 0), command.get("speed", 1.0)
        )
    except Exception as error:  # 单句失败不能拖垮进程。
        message = str(error) if error.args else "自然人声合成失败，可继续跟读"
        emit("error", id=request, message=message)
        return
    try:
        if not session.start(request, samples, rate):
            return
    except Exception:
        emit("error", id=request, message="无法播放示范朗读，请检查默认扬声器")
        return
    session.player.wait(len(samples) / rate, lambda: session.cancelled(request))
    if not session.cancelled(request):
        emit("done", id=request)


def run(voice, session, lines, idle_seconds=IDLE_SECONDS):
    while True:
        try:
            command = lines.get(timeout=idle_seconds)
        except queue.Empty:
            return
        if command is None:
            return
        speak(voice, session, command)


def main():
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    sys.stdin.reconfigure(encoding="utf-8", errors="replace")
    parser = argparse.ArgumentParser()
    parser.add_argument("--models", type=Path, default=Path(__file__).resolve().parents[1] / "models")
    args = parser.parse_args()
    # 与识别进程同理：先在主线程导入原生库并加载模型，再起读取标准输入的线程，避免 Windows 匿名管道卡住初始化。
    try:
        import sherpa_onnx  # noqa: F401
        from speech import NaturalVoice

        voice = NaturalVoice(args.models / "speech" / "en")
        player = SoundPlayer()
    except Exception as error:
        emit("fatal", message=str(error) if error.args else "自然人声组件无法加载")
        return
    session = Session(player)
    lines = queue.Queue()
    threading.Thread(target=read_commands, args=(lines, session), daemon=True).start()
    emit("ready")
    run(voice, session, lines)
    player.stop()


if __name__ == "__main__":
    main()
