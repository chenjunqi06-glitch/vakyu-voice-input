"""模型常驻而音频按会话存在；会话取消后丢弃队列中的旧结果。"""

import json
import queue
import sys
import threading

from capture import Capture
from dictation_context import limit_vocabulary


def start_options(request):
    """旧启动消息保留原默认值；语言与提示只属于本次录音。"""
    practice_text = request.get("practice_text")
    if not isinstance(practice_text, str):
        practice_text = None
    elif practice_text:
        practice_text = practice_text[:8192]
    language = request.get("language", "en" if practice_text else "zh")
    if language not in ("zh", "en"):
        raise ValueError("unsupported recognition language")
    vocabulary = None if practice_text else limit_vocabulary(request.get("dictation_vocabulary"))
    return practice_text, language, vocabulary


def run(models, emit):
    events = queue.Queue()
    capture = Capture(events, emit)

    def control():
        try:
            for line in sys.stdin:
                try:
                    request = json.loads(line)
                    command = request["command"]
                    generation = int(request.get("generation", 0))
                    if command not in ("start", "stop", "cancel", "shutdown"):
                        continue
                    options = start_options(request) if command == "start" else (None, None, None)
                    capture.command(command, generation, *options)
                    if command == "shutdown":
                        return
                except (ValueError, KeyError, TypeError):
                    print("忽略无效语音控制消息", file=sys.stderr, flush=True)
        finally:
            capture.command("shutdown", 0)

    try:
        # Windows 的 NumPy 原生模块首次导入不能与匿名管道的阻塞读取并行。
        # 先加载原生依赖，再启动控制线程；耗时的模型构造仍保持可取消。
        from recognition import Recognition, create_recognizer
    except Exception:
        emit("error", message="中文语音依赖加载失败，请修复语音组件")
        return
    threading.Thread(target=control, name="voice-control", daemon=True).start()
    try:
        recognizer = create_recognizer(models)
    except Exception:
        emit("error", message="中文语音模型加载失败，请修复语音组件")
        capture.command("shutdown", 0)
        return
    if not capture.alive:
        return
    emit("ready")
    active = None
    session = None
    latest = ""
    try:
        while capture.alive:
            kind, generation, payload = events.get()
            if kind == "shutdown":
                break
            if kind == "cancel":
                active, session = None, None
                continue
            if kind == "start":
                active, session, latest = generation, None, ""
                if not capture.requested(generation):
                    continue
                try:
                    practice_text, language, vocabulary = payload
                    session = Recognition(recognizer, practice_text=practice_text, language=language,
                                          dictation_vocabulary=vocabulary)
                    capture.open(generation)
                except Exception:
                    capture.cancel_if_current(generation)
                    emit("error", generation, message="无法打开默认麦克风，请检查输入设备与权限")
                continue
            if generation != active or capture.generation != generation:
                continue
            if kind == "overflow":
                # 丢音不可静默变成错误文字。
                capture.cancel_if_current(generation)
                emit("error", generation, message="麦克风输入过载，请关闭占用音频的应用后重试")
                continue
            if kind == "audio" and session is not None:
                text = session.accept(*payload)
                if capture.generation == generation and capture.held and text != latest:
                    latest = text
                    emit("interim", generation, text=text)
            elif kind == "stop":
                text = session.finish() if session is not None else ""
                if capture.generation == generation:
                    emit("final", generation, text=text)
                active, session = None, None
    except Exception:
        if active is not None:
            emit("error", active, message="中文语音识别中断，请重新按住 Alt")
    finally:
        capture.command("shutdown", 0)
