"""Vakyu 语音进程入口；标准输入输出只传会话控制与识别事件。"""

import argparse
import json
import sys
import threading
from pathlib import Path

OUTPUT_LOCK = threading.Lock()


def emit(event, generation=None, **fields):
    value = {"event": event, **fields}
    if generation is not None:
        value["generation"] = generation
    with OUTPUT_LOCK:
        print(json.dumps(value, ensure_ascii=False), flush=True)


def main():
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")
    sys.stdin.reconfigure(encoding="utf-8", errors="replace")
    parser = argparse.ArgumentParser()
    parser.add_argument("--models", type=Path, default=Path(__file__).resolve().parents[1] / "models")
    parser.add_argument("--wave", type=Path)
    parser.add_argument("--devices", action="store_true")
    args = parser.parse_args()
    if args.devices:
        from capture import list_devices
        print(json.dumps(list_devices(), ensure_ascii=False), flush=True)
        return
    if args.wave:
        from recognition import recognize_wave
        print(json.dumps(recognize_wave(args.models, args.wave), ensure_ascii=False), flush=True)
        return
    from worker import run
    run(args.models, emit)


if __name__ == "__main__":
    main()
