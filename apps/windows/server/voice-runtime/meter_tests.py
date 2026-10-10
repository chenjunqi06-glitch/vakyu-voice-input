"""真实音量映射、最新值聚合及停止后代次隔离验证；不访问麦克风。"""

import threading
import unittest
from unittest.mock import Mock

import numpy as np

from audio_meter import LevelMeter, audio_level


class AudioLevelTests(unittest.TestCase):
    def test_silence_and_sub_gate_noise_are_zero(self):
        self.assertEqual(audio_level(np.zeros(2400, dtype=np.float32)), 0)
        self.assertEqual(audio_level(np.full(2400, 0.001, dtype=np.float32)), 0)
        self.assertEqual(audio_level([]), 0)

    def test_real_volume_is_monotonic_and_bounded(self):
        levels = [audio_level(np.full(2400, amplitude, dtype=np.float32))
                  for amplitude in [0.0, 0.005, 0.02, 0.1, 0.3, 3.0]]
        self.assertEqual(levels[0], 0)
        self.assertEqual(levels[-1], 100)
        self.assertEqual(levels, sorted(levels))
        self.assertEqual(len(set(levels[:-1])), len(levels[:-1]))

    def test_nonfinite_samples_do_not_produce_a_fake_peak(self):
        self.assertEqual(audio_level([float("nan"), float("inf"), -float("inf")]), 0)
        level = audio_level([float("nan"), 0.02, -0.02])
        self.assertGreater(level, 0)
        self.assertLessEqual(level, 100)


class LevelMeterTests(unittest.TestCase):
    def setUp(self):
        self.emit = Mock()
        self.current = 7
        self.meter = LevelMeter(self.emit, lambda generation: generation == self.current,
                                threaded=False)
        self.meter.begin(7)

    def tearDown(self):
        self.meter.close()

    def test_only_latest_actual_audio_is_published_once(self):
        self.meter.offer(7, [0.01])
        self.meter.offer(7, [0.1])
        self.emit.assert_not_called()
        self.meter.publish()
        self.meter.publish()
        self.emit.assert_called_once_with("level", 7, level=audio_level([0.1]))

    def test_worker_waits_without_polling_when_idle_and_wakes_for_recording(self):
        self.meter.clear()
        self.meter.wake = Mock()
        transitions = iter([lambda: self.meter.begin(7), self.meter.clear, self.meter.close])
        self.meter.wake.wait.side_effect = lambda timeout: next(transitions)()
        self.meter._run()
        self.assertEqual([call.args[0] for call in self.meter.wake.wait.call_args_list],
                         [None, 1.0 / 30.0, None])
        self.emit.assert_not_called()

    def test_new_blocks_refresh_constant_volume_without_replaying_old_audio(self):
        for _ in range(6):
            self.meter.offer(7, [0.02])
            self.meter.publish()
        self.assertEqual(self.emit.call_count, 6)
        for _ in range(6):
            self.meter.publish()
        self.assertEqual(self.emit.call_count, 6)

    def test_cancel_discards_pending_old_generation(self):
        self.meter.offer(7, [0.1])
        self.current = None
        self.meter.clear()
        self.meter.publish()
        self.meter.offer(7, [0.1])
        self.meter.publish()
        self.emit.assert_not_called()

    def test_close_clears_reported_level_and_new_session_cannot_reuse_it(self):
        self.meter.offer(7, [0.1])
        self.meter.publish()
        self.current = None
        self.meter.clear()
        self.meter.publish()
        self.assertEqual(self.emit.call_args.args, ("level", 7))
        self.assertEqual(self.emit.call_args.kwargs, {"level": 0})
        self.emit.reset_mock()
        self.current = 8
        self.meter.begin(8)
        self.meter.offer(7, [0.9])
        self.meter.publish()
        self.emit.assert_not_called()
        self.meter.offer(8, [0.02])
        self.meter.publish()
        self.emit.assert_called_once_with("level", 8, level=audio_level([0.02]))

    def test_stale_recording_state_is_checked_before_sending(self):
        self.meter.offer(7, [0.1])
        self.current = None
        self.meter.publish()
        self.emit.assert_not_called()

    def test_slow_output_does_not_hold_audio_or_cancel_lock(self):
        entered, release = threading.Event(), threading.Event()

        def slow_emit(*args, **kwargs):
            entered.set()
            release.wait(2)

        self.meter.emit = slow_emit
        self.meter.offer(7, [0.1])
        publisher = threading.Thread(target=self.meter.publish)
        publisher.start()
        self.assertTrue(entered.wait(1))
        self.meter.offer(7, [0.02])
        self.meter.clear()
        self.assertIsNone(self.meter.state[1])
        release.set()
        publisher.join(2)
        self.assertFalse(publisher.is_alive())
