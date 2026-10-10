"""协议边界、停止排空、长录音及取消时的跨代次隔离。"""

import base64
from pathlib import Path
import sys
import threading
import time
import unittest

import numpy as np

sys.path.append(str(Path(__file__).resolve().parents[2] / "windows/server/voice-runtime"))
from channel import InputChannel
from external_main import process
from pcm import decode_audio


def packet(generation=1, value=0.125):
    return {"command": "audio", "generation": generation, "sample_rate": 16000,
            "pcm": base64.b64encode(np.full(800, value, dtype="<f4").tobytes()).decode()}


class FakeSession:
    def __init__(self, recognizer, practice_text=None, language=None):
        self.count = 0
        self.recognizer = recognizer

    def accept(self, rate, samples):
        self.count += 1
        if callable(self.recognizer):
            self.recognizer()
        return f"word {self.count}"

    def finish(self):
        return f"complete {self.count}"


class RuntimeTests(unittest.TestCase):
    def setUp(self):
        self.output = []
        self.channel = InputChannel(lambda event, generation=None, **fields:
                                    self.output.append((event, generation, fields)))
        self.thread = None

    def tearDown(self):
        self.channel.submit({"command": "shutdown"})
        if self.thread:
            self.thread.join(2)
            self.assertFalse(self.thread.is_alive())

    def start_processor(self, recognizer=None):
        self.thread = threading.Thread(target=process,
                                       args=(self.channel, recognizer, FakeSession))
        self.thread.start()

    def wait_for(self, event, generation=1):
        limit = time.monotonic() + 3
        while time.monotonic() < limit:
            found = [item for item in self.output if item[:2] == (event, generation)]
            if found:
                return found[-1][2]
            time.sleep(0.002)
        self.fail(f"missing {event} for {generation}")

    def start(self, generation=1):
        self.channel.submit({"command": "start", "generation": generation, "language": "en"})

    def test_float_pcm_roundtrip(self):
        rate, audio = decode_audio(packet())
        self.assertEqual(rate, 16000)
        self.assertEqual(audio.shape, (800,))
        self.assertTrue(np.all(audio == 0.125))

    def test_invalid_samples_and_packets(self):
        for value in (float("nan"), float("inf")):
            with self.assertRaises(ValueError):
                decode_audio(packet(value=value))
        for changed in ({"pcm": "###"}, {"sample_rate": True}, {"pcm": "AAAA"}):
            with self.assertRaises(ValueError):
                decode_audio({**packet(), **changed})

    def test_stop_drains_every_queued_block(self):
        self.start()
        for _ in range(20):
            self.channel.submit(packet())
        self.channel.submit({"command": "stop", "generation": 1})
        self.start_processor()
        self.assertEqual(self.wait_for("final")["text"], "complete 20")

    def test_cancel_during_decode_cannot_emit_old_text(self):
        entered, resume = threading.Event(), threading.Event()
        def slow():
            entered.set()
            resume.wait(2)
        self.start_processor(slow)
        self.start()
        self.wait_for("started")
        self.channel.submit(packet())
        self.assertTrue(entered.wait(2))
        self.channel.submit({"command": "cancel", "generation": 1})
        self.start(2)
        resume.set()
        self.wait_for("started", 2)
        self.assertFalse(any(event in ("interim", "final") and gen == 1
                             for event, gen, _ in self.output))

    def test_stale_stop_cannot_end_new_round(self):
        self.start(2)
        self.channel.submit({"command": "stop", "generation": 1})
        self.assertTrue(self.channel.accepting)
        self.assertEqual(self.channel.generation, 2)

    def test_overflow_fails_instead_of_silently_dropping_audio(self):
        self.start()
        for _ in range(300):
            self.channel.submit(packet())
        self.assertFalse(self.channel.accepting)
        self.assertTrue(any(event == "error" for event, _, _ in self.output))

    def test_long_recording_has_no_duration_cutoff(self):
        self.start_processor()
        self.start()
        self.wait_for("started")
        # 超过 1,000 个连续块，中间不断句停机；输入队列随处理保持有界。
        for index in range(1200):
            while self.channel.events.qsize() >= 128:
                time.sleep(0.001)
            self.channel.submit(packet())
        self.channel.submit({"command": "stop", "generation": 1})
        self.assertEqual(self.wait_for("final")["text"], "complete 1200")

    def test_invalid_audio_cancels_only_its_round(self):
        self.start()
        self.channel.submit({**packet(), "pcm": "invalid"})
        self.assertFalse(self.channel.current(1))
        self.start(2)
        self.channel.submit({**packet(), "pcm": "invalid"})
        self.assertTrue(self.channel.current(2))

    def test_level_is_real_amplitude(self):
        self.start()
        self.channel.submit(packet(value=0))
        self.start_processor()
        self.assertEqual(self.wait_for("level")["level"], 0)


if __name__ == "__main__":
    unittest.main()
