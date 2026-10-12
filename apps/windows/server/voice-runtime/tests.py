"""不开麦的控制竞态验证；真实声音准确率由独立 WAV 回放评估。"""

import io
import queue
import threading
import unittest
from unittest.mock import Mock, patch

import numpy as np

from audio_meter import LevelMeter, audio_level
from capture import Capture
from recognition import join_segments
from recognition_tests import RecognitionTests
from context_tests import PracticeContextTests, VocabTests
from meter_tests import AudioLevelTests, LevelMeterTests
import worker


class CaptureTests(unittest.TestCase):
    def setUp(self):
        self.events = queue.Queue()
        self.emit = Mock()
        self.capture = Capture(self.events, self.emit)

    def tearDown(self):
        self.capture.command("shutdown", 0)

    def test_release_during_model_load_never_opens_microphone(self):
        self.capture.command("start", 7)
        self.capture.command("stop", 7)
        with patch("capture.sd.InputStream") as microphone:
            self.assertFalse(self.capture.open(7))
            microphone.assert_not_called()

    def test_release_during_device_initialization_does_not_start_stream(self):
        self.capture.command("start", 7)
        stream = Mock()

        def initialize(**kwargs):
            self.capture.command("stop", 7)
            self.emit.assert_not_called()
            return stream

        with patch("capture.sd.query_devices", return_value={"default_samplerate": 48000}), \
                patch("capture.sd.InputStream", side_effect=initialize):
            self.assertFalse(self.capture.open(7))
        stream.start.assert_not_called()
        stream.close.assert_called_once()
        self.emit.assert_called_once_with("stopped", 7)

    def test_old_failure_cannot_cancel_a_new_recording(self):
        self.capture.command("start", 7)
        self.capture.command("cancel", 7)
        self.capture.command("start", 8)
        self.capture.cancel_if_current(7)
        self.assertTrue(self.capture.requested(8))
        self.assertFalse(self.capture.requested(7))

    def test_practice_context_stays_with_its_generation(self):
        self.capture.command("start", 7, "Chen Junqi")
        self.capture.command("cancel", 7)
        self.capture.command("start", 8)
        self.assertEqual([self.events.get_nowait() for _ in range(3)], [
            ("start", 7, ("Chen Junqi", "en")), ("cancel", 7, None), ("start", 8, (None, "zh")),
        ])
        self.assertFalse(self.capture.requested(7))
        self.assertTrue(self.capture.requested(8))

    def test_reverse_direction_carries_language_with_generation(self):
        for generation, request in [
            (7, {"language": "en"}),
            (8, {"language": "zh", "practice_text": "我今天去吃饭"}),
            (9, {}),
        ]:
            text, language = worker.start_options(request)
            self.capture.command("start", generation, text, language)
            self.assertEqual(self.events.get_nowait(), ("start", generation, (text, language)))
            self.assertTrue(self.capture.requested(generation))
        self.assertEqual(worker.start_options({}), (None, "zh"))
        self.assertEqual(worker.start_options({"practice_text": "Hello"}), ("Hello", "en"))
        with self.assertRaises(ValueError):
            worker.start_options({"language": "unsupported"})

    def test_microphone_callback_updates_meter_without_waiting_for_decoder(self):
        self.capture.meter.close()
        self.capture.meter = LevelMeter(self.emit, self.capture.requested, threaded=False)
        self.capture.command("start", 7)
        with patch("capture.sd.query_devices", return_value={"default_samplerate": 48000}), \
                patch("capture.sd.InputStream", return_value=Mock()) as microphone:
            self.assertTrue(self.capture.open(7))
        self.emit.reset_mock()
        callback = microphone.call_args.kwargs["callback"]
        audio = np.full((2400, 1), 0.02, dtype=np.float32)
        callback(audio, len(audio), None, Mock(input_overflow=False))
        self.emit.assert_not_called()
        self.capture.meter.publish()
        self.emit.assert_called_once_with("level", 7, level=audio_level(audio))
        self.assertEqual(self.events.qsize(), 2)
        self.events.get_nowait()
        kind, generation, (rate, samples) = self.events.get_nowait()
        self.assertEqual((kind, generation, rate), ("audio", 7, 48000))
        np.testing.assert_array_equal(samples, audio[:, 0])

    def test_stop_releases_device_before_inference_queue_is_consumed(self):
        self.capture.command("start", 7)
        stream = Mock()
        self.capture.stream = stream
        self.capture.command("stop", 7)
        stream.abort.assert_called_once()
        stream.close.assert_called_once()
        self.assertFalse(self.capture.held)

    def test_long_hold_does_not_stop_until_release(self):
        # 让旧的总时长定时器立即到期，等价于推进至其截止时刻。
        def expired_timer(interval, function, args):
            timer = Mock()
            timer.start.side_effect = lambda: function(*args)
            return timer

        self.capture.command("start", 7)
        stream = Mock()
        with patch("capture.sd.query_devices", return_value={"default_samplerate": 48000}), \
                patch("capture.sd.InputStream", return_value=stream), \
                patch("capture.threading.Timer", side_effect=expired_timer):
            self.assertTrue(self.capture.open(7))
        self.assertTrue(self.capture.requested(7))
        stream.close.assert_not_called()
        self.capture.command("stop", 7)
        stream.close.assert_called_once()

    def test_in_flight_audio_is_queued_before_stop(self):
        self.capture.command("start", 7)
        stream = Mock()
        stream.abort.side_effect = lambda: self.events.put(("audio", 7, "tail"))
        self.capture.stream = stream
        self.capture.command("stop", 7)
        self.assertEqual([self.events.get_nowait() for _ in range(3)], [
            ("start", 7, (None, "zh")), ("audio", 7, "tail"), ("stop", 7, None),
        ])

    def test_abort_error_still_closes_before_acknowledging(self):
        self.capture.command("start", 7)
        stream = Mock()
        stream.abort.side_effect = RuntimeError("device failure")
        self.capture.stream = stream
        self.capture.command("stop", 7)
        stream.close.assert_called_once()
        self.emit.assert_called_once_with("stopped", 7)

    def test_close_error_does_not_acknowledge_or_open_another_microphone(self):
        self.capture.command("start", 7)
        stream = Mock()
        stream.close.side_effect = RuntimeError("device failure")
        self.capture.stream = stream
        self.capture.command("stop", 7)
        self.capture.command("cancel", 7)
        self.capture.command("start", 8)
        self.emit.assert_not_called()
        self.assertFalse(self.capture.requested(8))

    def test_release_during_native_start_revokes_intent_and_defers_close_ack(self):
        self.capture.command("start", 7)
        entered, release = threading.Event(), threading.Event()
        stream = Mock()

        def start():
            entered.set()
            release.wait(2)

        stream.start.side_effect = start
        with patch("capture.sd.query_devices", return_value={"default_samplerate": 48000}), \
                patch("capture.sd.InputStream", return_value=stream):
            opening = threading.Thread(target=self.capture.open, args=(7,))
            opening.start()
            self.assertTrue(entered.wait(1))
            self.capture.command("stop", 7)
            self.assertFalse(self.capture.held)
            stream.close.assert_not_called()
            self.emit.assert_not_called()
            release.set()
            opening.join(2)
        self.assertFalse(opening.is_alive())
        stream.abort.assert_called_once()
        stream.close.assert_called_once()
        self.emit.assert_called_once_with("stopped", 7)

    def test_eof_closes_microphone(self):
        stream = Mock()
        self.capture.stream = stream
        with patch("worker.Capture", return_value=self.capture), \
                patch("sys.stdin", io.StringIO("")), \
                patch("recognition.create_recognizer", return_value=Mock()):
            worker.run(None, self.emit)
        stream.abort.assert_called_once()
        stream.close.assert_called_once()
        self.assertFalse(self.capture.alive)

    def test_segments_preserve_english_word_boundaries(self):
        self.assertEqual(join_segments("hello", "world"), "hello world")
        self.assertEqual(join_segments("今天", "吃饭"), "今天吃饭")


if __name__ == "__main__":
    unittest.main()
