"""macOS 原生采音入口；复用 Windows 的模型识别，不访问麦克风或写录音。"""

import argparse
import json
from pathlib import Path
import sys
import threading
import time

# 源码运行时从单一实现读取；打包时复制这四个共享模块到本目录。
SOURCE_RUNTIME = Path(__file__).resolve().parents[2] / "windows/server/voice-runtime"
if SOURCE_RUNTIME.is_dir():
    sys.path.append(str(SOURCE_RUNTIME))

OUTPUT_LOCK = threading.Lock()


def emit(event, generation=None, **fields):
    value = {"event": event, **fields}
    if generation is not None:
        value["generation"] = generation
    with OUTPUT_LOCK:
        print(json.dumps(value, ensure_ascii=False), flush=True)


def process(channel, recognizer, session_factory):
    from audio_meter import audio_level
    active, session, latest = None, None, ""
    last_meter = 0.0
    while channel.alive:
        kind, generation, payload = channel.events.get()
        if kind == "shutdown":
            break
        if kind == "cancel":
            active, session, latest = None, None, ""
            continue
        if not channel.current(generation):
            continue
        try:
            if kind == "start":
                hint, language = payload
                active, latest = generation, ""
                session = session_factory(recognizer, practice_text=hint, language=language)
                if channel.current(generation):
                    channel.emit("started", generation)
            elif generation == active and session is not None:
                if kind == "audio":
                    rate, samples = payload
                    text = session.accept(rate, samples)
                    if channel.current(generation):
                        now = time.monotonic()
                        if now - last_meter >= 1 / 30:
                            last_meter = now
                            channel.emit("level", generation, level=audio_level(samples) / 100.0)
                        if text != latest:
                            latest = text
                            channel.emit("interim", generation, text=text)
                elif kind == "stop":
                    text = session.finish()
                    if channel.current(generation):
                        channel.emit("level", generation, level=0)
                        channel.emit("final", generation, text=text)
                    channel.invalidate(generation)
                    active, session, latest = None, None, ""
        except Exception:
            if channel.current(generation):
                channel.emit("error", generation, message="语音识别中断，请重新按住快捷键")
            channel.invalidate(generation)
            active, session, latest = None, None, ""


def main():
    for stream in (sys.stdin, sys.stdout, sys.stderr):
        stream.reconfigure(encoding="utf-8", errors="replace")
    parser = argparse.ArgumentParser()
    parser.add_argument("--models", type=Path, required=True)
    parser.add_argument("--check-model", action="store_true")
    args = parser.parse_args()
    try:
        from channel import InputChannel
        from recognition import Recognition, create_recognizer
        recognizer = create_recognizer(args.models)
    except Exception:
        emit("error", message="语音模型或组件加载失败，请修复 Vakyu 语音组件")
        return 1
    emit("ready")
    if args.check_model:
        return 0
    channel = InputChannel(emit)
    threading.Thread(target=channel.read, args=(sys.stdin,), daemon=True,
                     name="native-audio-control").start()
    process(channel, recognizer, Recognition)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
