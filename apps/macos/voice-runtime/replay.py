"""以真实 WAV 回放验证 external PCM 入口；一次子进程只加载一份模型。"""

import argparse
import base64
import json
from pathlib import Path
import queue
import subprocess
import sys
import threading
import time
import wave

import numpy as np


def replay(python, models, waves):
    runtime = Path(__file__).resolve().parent
    # Windows 嵌入式 Python 不自动将脚本目录加入 sys.path。
    bootstrap = "import sys,runpy;sys.path.insert(0,sys.argv.pop(1));runpy.run_module('external_main',run_name='__main__')"
    child = subprocess.Popen([str(python), "-u", "-c", bootstrap, str(runtime),
                              "--models", str(models)], stdin=subprocess.PIPE,
                             stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                             text=True, encoding="utf-8")
    events, errors = queue.Queue(), []
    def receive():
        for line in child.stdout:
            events.put(json.loads(line))
    def stderr():
        for line in child.stderr:
            errors.append(line.rstrip())
    threading.Thread(target=receive, daemon=True).start()
    threading.Thread(target=stderr, daemon=True).start()
    def send(value):
        child.stdin.write(json.dumps(value) + "\n")
        child.stdin.flush()
    def await_event(expected, generation=None):
        deadline = time.monotonic() + 60
        while time.monotonic() < deadline:
            event = events.get(timeout=max(0.1, deadline - time.monotonic()))
            if event["event"] == "error":
                raise RuntimeError(event["message"])
            if event["event"] == expected and event.get("generation") == generation:
                return event
        raise TimeoutError(expected)
    reports = []
    try:
        await_event("ready")
        for generation, filename in enumerate(waves, 1):
            with wave.open(str(filename), "rb") as source:
                assert source.getsampwidth() == 2
                rate, channels = source.getframerate(), source.getnchannels()
                data = np.frombuffer(source.readframes(source.getnframes()), dtype="<i2")
                data = data.astype(np.float32).reshape(-1, channels).mean(axis=1) / 32768
            language = "en" if "english" in filename.name else "zh"
            send({"command": "start", "generation": generation, "language": language})
            await_event("started", generation)
            before = time.monotonic()
            for offset in range(0, len(data), rate // 20):
                chunk = data[offset:offset + rate // 20].astype("<f4").tobytes()
                send({"command": "audio", "generation": generation, "sample_rate": rate,
                      "pcm": base64.b64encode(chunk).decode()})
                time.sleep(0.01)
            send({"command": "stop", "generation": generation})
            event = await_event("final", generation)
            text = event["text"]
            if "silence" in filename.name:
                assert not text, "静音不能输出文字"
            else:
                assert text.strip(), "有效语音必须有结果"
            reports.append({"file": filename.name, "language": language, "text": text,
                            "audio_seconds": len(data) / rate,
                            "elapsed_seconds": round(time.monotonic() - before, 3)})
        send({"command": "shutdown"})
        child.wait(timeout=10)
        assert child.returncode == 0
        return {"model_instances": 1, "fixtures": reports, "stderr_lines": len(errors)}
    finally:
        if child.poll() is None:
            child.terminate()
            child.wait(timeout=10)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--python", type=Path, default=Path(sys.executable))
    parser.add_argument("--models", type=Path, required=True)
    parser.add_argument("--output", type=Path)
    parser.add_argument("waves", nargs="+", type=Path)
    args = parser.parse_args()
    result = json.dumps(replay(args.python, args.models, args.waves), ensure_ascii=False, indent=2)
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(result + "\n", encoding="utf-8")
    print(result)
