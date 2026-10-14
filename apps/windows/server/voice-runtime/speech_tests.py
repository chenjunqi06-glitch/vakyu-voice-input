"""自然人声的文本整理、取消与协议；用假模型和假播放器，不需要 sherpa-onnx 或声卡。"""

import io
import json
import queue
import tempfile
import threading
import unittest
from contextlib import redirect_stdout
from pathlib import Path

import speak_main
from speech import NaturalVoice, SpeechError, clamp_speed, clean_text, model_files_present


class FakeAudio:
    def __init__(self, samples, sample_rate):
        self.samples = samples
        self.sample_rate = sample_rate


class FakeEngine:
    num_speakers = 904

    def __init__(self):
        self.calls = []

    def generate(self, text, sid=0, speed=1.0):
        self.calls.append((text, sid, speed))
        return FakeAudio([0.0] * 2205, 22050)


class FakePlayer:
    def __init__(self):
        self.played = []
        self.stops = 0
        self.on_wait = None

    def play(self, samples, rate):
        self.played.append((len(samples), rate))

    def wait(self, seconds, cancelled):
        if self.on_wait:
            self.on_wait()

    def stop(self):
        self.stops += 1


def model_root(directory):
    root = Path(directory)
    (root / "espeak-ng-data").mkdir(parents=True)
    for name in ("model.onnx", "tokens.txt", "espeak-ng-data/phontab", "espeak-ng-data/en_dict"):
        (root / name).write_bytes(b"x")
    return root


def voice(directory):
    engine = FakeEngine()
    return NaturalVoice(model_root(directory), engine_factory=lambda root, threads: engine), engine


def events(output):
    return [json.loads(line) for line in output.getvalue().splitlines()]


class SpeechTextTests(unittest.TestCase):
    def test_clean_text_removes_control_characters_and_limits_length(self):
        self.assertEqual(clean_text("  Hello\x00\tworld\n "), "Hello world")
        self.assertEqual(clean_text(None), "")
        self.assertEqual(len(clean_text("a" * 5000)), 600)

    def test_speed_is_clamped(self):
        self.assertEqual(clamp_speed(0.1), 0.5)
        self.assertEqual(clamp_speed(9), 2.0)
        self.assertEqual(clamp_speed("bad"), 1.0)
        self.assertEqual(clamp_speed(float("nan")), 1.0)
        self.assertAlmostEqual(clamp_speed(0.8), 0.8)

    def test_model_files_present(self):
        with tempfile.TemporaryDirectory() as directory:
            self.assertFalse(model_files_present(directory))
            self.assertTrue(model_files_present(model_root(directory)))


class SpeechVoiceTests(unittest.TestCase):
    def test_missing_model_is_reported(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaises(SpeechError):
                NaturalVoice(directory, engine_factory=lambda root, threads: FakeEngine())

    def test_speaker_out_of_range_falls_back_to_first(self):
        with tempfile.TemporaryDirectory() as directory:
            natural, engine = voice(directory)
            natural.synthesize("Hello.", speaker=20, speed=0.8)
            natural.synthesize("Hello.", speaker=5000, speed=0.8)
            natural.synthesize("Hello.", speaker="x", speed=0.8)
            self.assertEqual([call[1] for call in engine.calls], [20, 0, 0])

    def test_empty_text_is_an_error(self):
        with tempfile.TemporaryDirectory() as directory:
            natural, _ = voice(directory)
            with self.assertRaises(SpeechError):
                natural.synthesize(" \n ")


class SpeechProtocolTests(unittest.TestCase):
    def run_lines(self, natural, player, lines):
        session = speak_main.Session(player)
        commands = queue.Queue()
        output = io.StringIO()
        with redirect_stdout(output):
            for line in lines:
                speak_main.dispatch(json.dumps(line), commands, session)
            commands.put(None)
            speak_main.run(natural, session, commands, idle_seconds=0.1)
        return events(output), session

    def test_speaks_in_order_and_reports_done(self):
        with tempfile.TemporaryDirectory() as directory:
            natural, engine = voice(directory)
            player = FakePlayer()
            result, _ = self.run_lines(natural, player, [
                {"command": "speak", "id": 1, "text": "Hello.", "speaker": 20, "speed": 0.8},
                {"command": "speak", "id": 2, "text": "Good morning.", "speaker": 20, "speed": 1.0},
            ])
            self.assertEqual(result, [{"event": "done", "id": 1}, {"event": "done", "id": 2}])
            self.assertEqual(len(player.played), 2)
            self.assertEqual(engine.calls[0], ("Hello.", 20, 0.8))

    def test_stop_cancels_everything_received_before_it(self):
        with tempfile.TemporaryDirectory() as directory:
            natural, _ = voice(directory)
            player = FakePlayer()
            result, _ = self.run_lines(natural, player, [
                {"command": "speak", "id": 1, "text": "Old."},
                {"command": "stop"},
                {"command": "speak", "id": 2, "text": "New."},
            ])
            self.assertEqual(result, [{"event": "done", "id": 2}])
            self.assertEqual(len(player.played), 1)

    def test_stop_during_playback_suppresses_done(self):
        with tempfile.TemporaryDirectory() as directory:
            natural, _ = voice(directory)
            player = FakePlayer()
            session = speak_main.Session(player)
            player.on_wait = session.stop
            session.seen(1)
            output = io.StringIO()
            with redirect_stdout(output):
                speak_main.speak(natural, session, {"command": "speak", "id": 1, "text": "Hi."})
            self.assertEqual(events(output), [])
            self.assertEqual(player.stops, 1)

    def test_synthesis_failure_is_reported_for_that_request(self):
        with tempfile.TemporaryDirectory() as directory:
            natural, _ = voice(directory)
            result, _ = self.run_lines(natural, FakePlayer(), [
                {"command": "speak", "id": 3, "text": "   "},
                {"command": "speak", "id": 4, "text": "Fine."},
            ])
            self.assertEqual(result[0]["event"], "error")
            self.assertEqual(result[0]["id"], 3)
            self.assertEqual(result[1], {"event": "done", "id": 4})

    def test_playback_failure_is_reported(self):
        class BrokenPlayer(FakePlayer):
            def play(self, samples, rate):
                raise OSError("no device")

        with tempfile.TemporaryDirectory() as directory:
            natural, _ = voice(directory)
            result, _ = self.run_lines(natural, BrokenPlayer(), [{"command": "speak", "id": 5, "text": "Hi."}])
            self.assertEqual(result, [{"event": "error", "id": 5, "message": "无法播放示范朗读，请检查默认扬声器"}])

    def test_shutdown_and_garbage_lines(self):
        session = speak_main.Session(FakePlayer())
        commands = queue.Queue()
        self.assertTrue(speak_main.dispatch("garbage", commands, session))
        self.assertTrue(speak_main.dispatch("[]", commands, session))
        self.assertTrue(speak_main.dispatch(json.dumps({"command": "speak", "id": "x"}), commands, session))
        self.assertFalse(speak_main.dispatch(json.dumps({"command": "shutdown"}), commands, session))
        self.assertTrue(commands.empty())

    def test_idle_exit(self):
        with tempfile.TemporaryDirectory() as directory:
            natural, _ = voice(directory)
            session = speak_main.Session(FakePlayer())
            done = threading.Event()
            thread = threading.Thread(target=lambda: (speak_main.run(natural, session, queue.Queue(), 0.05), done.set()))
            thread.start()
            thread.join(2)
            self.assertTrue(done.is_set())


if __name__ == "__main__":
    unittest.main()
