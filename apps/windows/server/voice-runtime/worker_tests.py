"""识别主循环：积压音频合并与偶发输入过载的处理，不开麦、不加载模型。"""

import collections
import io
import queue
import unittest
from unittest.mock import Mock, patch

import numpy as np

import worker


def audio(generation, length, rate=16000):
    return ("audio", generation, (rate, np.ones(length, dtype=np.float32)))


class MergeAudioTests(unittest.TestCase):
    def test_merges_backlog_until_next_control_event(self):
        events, pending = queue.Queue(), collections.deque()
        for event in (audio(3, 800), audio(3, 800), ("stop", 3, None), audio(3, 800)):
            events.put(event)
        rate, samples = worker.merge_audio((16000, np.ones(800, dtype=np.float32)), 3, events, pending)
        self.assertEqual((rate, len(samples)), (16000, 2400))
        self.assertEqual(list(pending), [("stop", 3, None)])
        self.assertEqual(worker.next_event(events, pending), ("stop", 3, None))
        self.assertEqual(worker.next_event(events, pending)[0], "audio")

    def test_never_crosses_generation_or_sample_rate(self):
        for other in (audio(4, 800), audio(3, 800, rate=48000)):
            events, pending = queue.Queue(), collections.deque()
            events.put(other)
            _, samples = worker.merge_audio((16000, np.ones(800, dtype=np.float32)), 3, events, pending)
            self.assertEqual(len(samples), 800)
            self.assertEqual(list(pending), [other])

    def test_merge_is_bounded(self):
        events, pending = queue.Queue(), collections.deque()
        for _ in range(100):
            events.put(audio(3, 800))
        _, samples = worker.merge_audio((16000, np.ones(800, dtype=np.float32)), 3, events, pending)
        self.assertEqual(len(samples), int(16000 * worker.MAX_MERGED_SECONDS))
        self.assertGreater(events.qsize(), 0)

    def test_does_not_overtake_a_pending_event(self):
        events, pending = queue.Queue(), collections.deque([("stop", 3, None)])
        events.put(audio(3, 800))
        _, samples = worker.merge_audio((16000, np.ones(800, dtype=np.float32)), 3, events, pending)
        self.assertEqual(len(samples), 800)
        self.assertEqual(events.qsize(), 1)


class FakeCapture:
    def __init__(self):
        self.alive, self.generation, self.held = True, 7, True
        self.cancelled = []

    def command(self, *args, **kwargs):
        pass

    def requested(self, generation):
        return generation == self.generation

    def open(self, generation):
        return True

    def cancel_if_current(self, generation):
        self.cancelled.append(generation)


class RunLoopTests(unittest.TestCase):
    def run_events(self, events):
        feed = queue.Queue()
        for event in events:
            feed.put(event)
        feed.put(("shutdown", 0, None))
        capture, emit = FakeCapture(), Mock()
        recognition = Mock()
        recognition.accept.side_effect = lambda rate, samples: f"{len(samples)}"
        recognition.finish.return_value = "done"
        with patch("worker.queue.Queue", return_value=feed), \
                patch("worker.Capture", return_value=capture), \
                patch("sys.stdin", io.StringIO("")), \
                patch("recognition.create_recognizer", return_value=Mock()), \
                patch("recognition.Recognition", return_value=recognition):
            worker.run(None, emit)
        return capture, emit, recognition

    def test_brief_overflow_keeps_recording(self):
        capture, emit, _ = self.run_events(
            [("start", 7, (None, "zh", None)), ("overflow", 7, None), audio(7, 800), ("stop", 7, None)])
        self.assertEqual(capture.cancelled, [])
        emit.assert_any_call("final", 7, text="done")
        self.assertNotIn("error", [call.args[0] for call in emit.call_args_list])

    def test_sustained_overflow_stops_with_error(self):
        events = [("start", 7, (None, "zh", None))] + [("overflow", 7, None)] * worker.MAX_OVERFLOWS
        capture, emit, _ = self.run_events(events)
        self.assertEqual(capture.cancelled, [7])
        self.assertEqual(emit.call_args_list[-1].args[:2], ("error", 7))

    def test_backlog_is_decoded_in_one_call_without_losing_samples(self):
        events = [("start", 7, (None, "zh", None))] + [audio(7, 800)] * 5 + [("stop", 7, None)]
        _, emit, recognition = self.run_events(events)
        self.assertEqual(recognition.accept.call_count, 1)
        self.assertEqual(len(recognition.accept.call_args.args[1]), 4000)
        emit.assert_any_call("final", 7, text="done")


if __name__ == "__main__":
    unittest.main()
