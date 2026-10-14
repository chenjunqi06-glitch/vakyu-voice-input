"""Vakyu 离线翻译进程入口：标准输入输出是 JSON 行，只传翻译请求与结果，不联网、不写日志。

请求：{"command":"translate","id":1,"text":"…","target":"en"|"zh"}、{"command":"shutdown"}。
回复：{"event":"ready"}、{"event":"translation","id":1,"ok":true,"text":"…"}，失败时 ok=false、message 为可读原因。
空闲 10 分钟自动退出释放内存，父进程下次翻译时重新启动。
"""

import argparse
import json
import queue
import sys
import threading
from pathlib import Path

IDLE_SECONDS = 600

OUTPUT_LOCK = threading.Lock()


def emit(event, **fields):
    with OUTPUT_LOCK:
        print(json.dumps({"event": event, **fields}, ensure_ascii=False), flush=True)


def read_commands(lines):
    for line in sys.stdin:
        lines.put(line)
    lines.put(None)


def handle(translator, command):
    """处理一条命令；返回 False 表示退出。"""
    kind = command.get("command")
    if kind == "shutdown":
        return False
    if kind != "translate":
        return True
    request = command.get("id")
    if not isinstance(request, int):
        return True
    try:
        text = translator.translate(command.get("text", ""), command.get("target"))
        emit("translation", id=request, ok=True, text=text)
    except Exception as error:  # 单条失败不能拖垮进程。
        message = str(error) if error.args else "离线翻译失败，请重试"
        emit("translation", id=request, ok=False, message=message)
    return True


def run(translator, lines, idle_seconds=IDLE_SECONDS):
    while True:
        try:
            line = lines.get(timeout=idle_seconds)
        except queue.Empty:
            return
        if line is None:
            return
        try:
            command = json.loads(line)
        except ValueError:
            continue
        if not isinstance(command, dict) or not handle(translator, command):
            return


def main():
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    sys.stdin.reconfigure(encoding="utf-8", errors="replace")
    parser = argparse.ArgumentParser()
    parser.add_argument("--models", type=Path, default=Path(__file__).resolve().parents[1] / "models")
    args = parser.parse_args()
    # 与识别进程同理：先在主线程导入原生库，再起读取标准输入的线程，避免 Windows 匿名管道卡住初始化。
    import ctranslate2  # noqa: F401
    import sentencepiece  # noqa: F401
    from translation import OfflineTranslator

    translator = OfflineTranslator(args.models / "translate")
    lines = queue.Queue()
    threading.Thread(target=read_commands, args=(lines,), daemon=True).start()
    emit("ready")
    run(translator, lines)


if __name__ == "__main__":
    main()
