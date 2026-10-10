"""服务进程管道回归：假识别器，不访问网络、麦克风或应用输入框。"""

import argparse
import json
from pathlib import Path
import queue
import subprocess
import sys
import tempfile
import threading
import time


FAKE = '''import json, sys
print(json.dumps({"event":"ready"}), flush=True)
for line in sys.stdin:
    cmd=json.loads(line)
    generation=cmd.get("generation", 0)
    if cmd["command"]=="start":
        print(json.dumps({"event":"started","generation":generation}), flush=True)
    elif cmd["command"]=="stop":
        print(json.dumps({"event":"final","generation":generation,"text":"Protocol test only"}), flush=True)
    elif cmd["command"]=="shutdown":
        break
'''


def run(executable):
    with tempfile.TemporaryDirectory(prefix="vakyu-protocol-") as temporary:
        root = Path(temporary)
        (root / "external_main.py").write_text(FAKE, encoding="utf-8")
        config = root / "config.toml"
        config.write_text('[shortcut]\nvoice_english_practice = true\nvoice_practice_learn_english = false\n'
                          '[predict]\napi_key = "test-only-never-used"\n', encoding="utf-8")
        process = subprocess.Popen(
            [executable, "--config", str(config), "--data", str(root / "data"),
             "--python", sys.executable, "--runtime", str(root), "--models", str(root)],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            text=True, encoding="utf-8",
        )
        events = queue.Queue()
        seen = []

        def read():
            for line in process.stdout:
                assert "test-only-never-used" not in line
                events.put(json.loads(line))

        threading.Thread(target=read, daemon=True).start()

        def send(command, **fields):
            process.stdin.write(json.dumps(dict(command=command, **fields)) + "\n")
            process.stdin.flush()

        def until(kind, predicate=lambda event: True):
            deadline = time.monotonic() + 10
            while time.monotonic() < deadline:
                event = events.get(timeout=max(0.01, deadline - time.monotonic()))
                seen.append(event)
                if event["event"] == kind and predicate(event):
                    return event
            raise AssertionError(f"Missing event {kind}")

        try:
            until("ready")
            send("press", mode="dictation")
            capture = until("capture", lambda event: event["active"])
            send("audio", generation=capture["generation"], sample_rate=16000, pcm="AAAAAA==")
            send("release")
            commit = until("commit")
            assert commit["text"] == "Protocol test only"
            send("commit_result", id=commit["id"], ok=True)
            until("hide")
            assert not any(event["event"] in ("practice", "speak") for event in seen)
            send("set", key="voice_english_practice", value=False)
            disabled = until("setting_result")
            assert disabled["ok"]
            start = len(seen)
            send("press", mode="practice")
            until("error")
            assert not any(event["event"] == "commit" or
                           (event["event"] == "capture" and event["active"])
                           for event in seen[start:])
            send("set", key="voice_practice_learn_english", value=False)
            saved = until("setting_result")
            assert saved["key"] == "voice_practice_learn_english" and saved["ok"]
            send("set", key="api_key", value="must-not-save")
            rejected = until("setting_result")
            assert not rejected["ok"]
            assert "must-not-save" not in config.read_text(encoding="utf-8")
            send("shutdown")
            assert process.wait(timeout=10) == 0
            assert len([event for event in seen if event["event"] == "commit"]) == 1
        finally:
            if process.poll() is None:
                process.kill()
                process.wait(timeout=10)
    print("PASS: explicit mode routing, disabled practice rejection, PCM forwarding, single confirmed commit, settings allowlist, clean shutdown")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("executable")
    run(parser.parse_args().executable)
