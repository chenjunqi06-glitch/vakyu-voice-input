"""中英混说词汇提示的输入边界、会话隔离与回退验证；不使用麦克风。"""

import queue
import re
import unittest
from unittest.mock import Mock

from capture import Capture
from dictation_context import MAX_HOTWORDS, MAX_TEXT, MAX_WORDS, dictation_hotwords
from recognition import Recognition
from worker import start_options


class DictationContextTests(unittest.TestCase):
    def test_personal_english_vocabulary_is_weak_and_keeps_case_variants(self):
        hints = dictation_hotwords("Codex deadline I'm").split("/")
        self.assertIn("Codex :0.8", hints)
        self.assertIn("codex :0.8", hints)
        self.assertIn("deadline :0.8", hints)
        self.assertIn("I'm :0.8", hints)
        self.assertTrue(all(float(item.rsplit(":", 1)[1]) == 0.8 for item in hints))

    def test_chinese_short_letters_and_invalid_inputs_do_not_force_english(self):
        for value in [None, 123, [], {}, "", "今天去吃饭", "a I x 123", "x" * 49]:
            self.assertEqual(dictation_hotwords(value), "")

    def test_delimiters_cannot_inject_scores_or_full_sentence_prompts(self):
        hints = dictation_hotwords("hello/:9999\nworld :1000 let's go").split("/")
        self.assertTrue(all(re.fullmatch(r"[A-Za-z]+(?:'[A-Za-z]+)? :0\.8", item) for item in hints))
        self.assertNotIn("hello world", "/".join(hints))

    def test_vocabulary_is_bounded_and_repetition_does_not_amplify_words(self):
        self.assertEqual(dictation_hotwords("Codex codex CODEX " * 500), dictation_hotwords("Codex"))
        words = ["word" + chr(97 + i // 26) + chr(97 + i % 26) for i in range(200)]
        hints = dictation_hotwords(" ".join(words))
        self.assertLessEqual(len(hints), MAX_HOTWORDS)
        self.assertEqual(len({item.rsplit(" :", 1)[0].lower() for item in hints.split("/")}), MAX_WORDS)
        self.assertEqual(dictation_hotwords(" " * MAX_TEXT + "Codex"), "")

    def test_only_current_dictation_stream_receives_its_vocabulary(self):
        recognizer = Mock(practice_context_enabled=True)
        Recognition(recognizer, language="zh", dictation_vocabulary="deadline Codex")
        self.assertIn("deadline :0.8", recognizer.create_stream.call_args.args[0])
        Recognition(recognizer)
        self.assertEqual(recognizer.create_stream.call_args.args, ())
        Recognition(recognizer, "Chen Junqi", language="en", dictation_vocabulary="deadline Codex")
        self.assertNotIn("deadline", recognizer.create_stream.call_args.args[0])
        self.assertIn("Chen Junqi :1.2", recognizer.create_stream.call_args.args[0])

    def test_case_variants_cannot_evict_personal_words_at_the_hotword_limit(self):
        base = (
            "file folder email message meeting deadline update feedback project report "
            "document download upload link website browser account password login app application "
            "Windows macOS iPhone Android Google Apple GitHub ChatGPT Codex Vakyu API AI PDF "
            "Word Excel PowerPoint bug code model server client database token prompt test version "
            "feature online offline English hello thanks sorry please okay music video photo "
            "coffee lunch dinner office team schedule plan design review support"
        )
        personal = [chr(97 + index) + "bC" + "d" * 45 for index in range(24)]
        vocabulary = base.split() + personal
        self.assertEqual(len(vocabulary), 93)
        hints = dictation_hotwords(" ".join(vocabulary))
        accepted = {item.rsplit(" :", 1)[0].lower() for item in hints.split("/")}
        self.assertEqual(accepted, {word.lower() for word in vocabulary})
        self.assertLessEqual(len(hints), MAX_HOTWORDS)
        expected = [f"{variant} :0.8" for word in base.split()
                    for variant in dict.fromkeys((word, word.lower(), word.capitalize()))]
        self.assertEqual(dictation_hotwords(base), "/".join(expected))

    def test_english_dictation_and_chinese_practice_keep_separate_contexts(self):
        recognizer = Mock(practice_context_enabled=True)
        Recognition(recognizer, language="en", dictation_vocabulary="deadline")
        self.assertIn("deadline :0.8", recognizer.create_stream.call_args.args[0])
        Recognition(recognizer, "我今天吃饭", language="zh", dictation_vocabulary="deadline")
        self.assertIn("我 :0.6", recognizer.create_stream.call_args.args[0])
        self.assertNotIn("deadline", recognizer.create_stream.call_args.args[0])

    def test_endpoint_reuses_hinted_stream_and_keeps_completed_text(self):
        recognizer = Mock(practice_context_enabled=True)
        recognizer.is_ready.return_value = False
        recognizer.is_endpoint.return_value = True
        recognizer.get_result.return_value = "已经说完。"
        session = Recognition(recognizer, dictation_vocabulary="deadline")
        stream = session.stream
        self.assertEqual(session.accept(16000, [0.0] * 800), "已经说完。")
        recognizer.reset.assert_called_once_with(stream)
        recognizer.is_endpoint.return_value = False
        recognizer.get_result.return_value = "deadline 是明天"
        self.assertEqual(session.accept(16000, [0.0] * 800), "已经说完。deadline 是明天")
        self.assertIs(session.stream, stream)
        recognizer.create_stream.assert_called_once_with("deadline :0.8/Deadline :0.8")

    def test_unsupported_model_or_context_failure_keeps_plain_dictation(self):
        recognizer = Mock(practice_context_enabled=False)
        Recognition(recognizer, dictation_vocabulary="deadline")
        recognizer.create_stream.assert_called_once_with()
        recognizer = Mock(practice_context_enabled=True)
        recognizer.create_stream.side_effect = [RuntimeError("context unavailable"), Mock()]
        Recognition(recognizer, dictation_vocabulary="deadline")
        self.assertEqual(recognizer.create_stream.call_count, 2)
        self.assertEqual(recognizer.create_stream.call_args.args, ())

    def test_start_options_limits_vocabulary_and_drops_it_for_practice(self):
        self.assertEqual(start_options({"dictation_vocabulary": "deadline"}), (None, "zh", "deadline"))
        self.assertEqual(start_options({"dictation_vocabulary": ["deadline"]}), (None, "zh", None))
        self.assertEqual(start_options({"practice_text": "Hello", "dictation_vocabulary": "deadline"}),
                         ("Hello", "en", None))
        self.assertLessEqual(len(start_options({"dictation_vocabulary": "Codex " * 4000})[2]), MAX_TEXT)
        self.assertEqual(start_options({"dictation_vocabulary": "x" * 20000})[2], "")

    def test_capture_cancellation_cannot_leak_vocabulary_into_next_generation(self):
        events = queue.Queue()
        capture = Capture(events, Mock())
        try:
            capture.command("start", 7, dictation_vocabulary="deadline")
            capture.command("cancel", 7)
            capture.command("start", 8)
            self.assertEqual([events.get_nowait() for _ in range(3)], [
                ("start", 7, (None, "zh", "deadline")), ("cancel", 7, None),
                ("start", 8, (None, "zh", None)),
            ])
        finally:
            capture.command("shutdown", 0)
