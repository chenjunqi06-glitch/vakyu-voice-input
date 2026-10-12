"""跟读上下文的隔离、弱提示、词表降级验证；不访问麦克风。"""

import hashlib
import tempfile
import unittest
from pathlib import Path
from unittest.mock import Mock, patch

from bpe_vocab import prepare_bpe_vocab
from practice_context import MAX_HOTWORDS, practice_hotwords
from recognition import Recognition, chinese_context_tokens, create_recognizer


class PracticeContextTests(unittest.TestCase):
    def test_chinese_context_is_weak_bounded_and_keeps_mixed_names(self):
        text = "我今天要去吃饭，然后和 Alice 一起散步。"
        hints = practice_hotwords(text, "zh").split("/")
        self.assertIn("我 :0.6", hints)
        self.assertIn("今 天 :1.0", hints)
        self.assertIn("Alice :1.0", hints)
        self.assertNotIn("我今天要去吃饭", "/".join(hints))
        self.assertNotIn("然", practice_hotwords(text, "en"))
        self.assertLessEqual(len(practice_hotwords(text * 1000, "zh")), MAX_HOTWORDS)

    def test_chinese_hotwords_use_individual_bpe_words_and_drop_unknown_characters(self):
        tokens = frozenset("我今天要去吃饭")
        hints = practice_hotwords("我今天要去吃饭，龘，天龘饭。", "zh", tokens)
        self.assertNotIn("龘", hints)
        self.assertIn("今 天 :1.0", hints.split("/"))
        for item in hints.split("/"):
            words = item.rsplit(" :", 1)[0].split()
            self.assertTrue(all(len(word) == 1 and word in tokens for word in words))
        self.assertEqual(practice_hotwords("龘", "zh", tokens), "")
        self.assertEqual(practice_hotwords("今天", "zh", frozenset()), "")
        self.assertIn("Alice :1.0", practice_hotwords("Alice 龘", "zh", tokens))

    def test_empty_supported_chinese_context_falls_back_to_ordinary_stream(self):
        recognizer = Mock(practice_context_enabled=True, practice_chinese_tokens=frozenset("今天"))
        Recognition(recognizer, "龘", language="zh")
        recognizer.create_stream.assert_called_once_with()
        Recognition(recognizer, "今天龘", language="zh")
        self.assertNotIn("龘", recognizer.create_stream.call_args.args[0])
        self.assertIn("今 天 :1.0", recognizer.create_stream.call_args.args[0])

    def test_chinese_practice_cannot_leak_hints_into_english_dictation(self):
        recognizer = Mock(practice_context_enabled=True)
        session = Recognition(recognizer, "我叫陈俊奇", language="zh")
        self.assertEqual(session.language, "zh")
        self.assertIn("陈 :0.6", recognizer.create_stream.call_args.args[0])
        session = Recognition(recognizer, language="en")
        self.assertEqual(session.language, "en")
        self.assertEqual(recognizer.create_stream.call_args.args, ())
        Recognition(recognizer)
        self.assertEqual(recognizer.create_stream.call_args.args, ())

    def test_words_and_short_name_have_low_scores_without_full_sentence(self):
        hints = practice_hotwords("My name is Chen Junqi, and I want lunch.").split("/")
        self.assertIn("Chen :1.0", hints)
        self.assertIn("chen :1.0", hints)
        self.assertIn("Chen Junqi :1.2", hints)
        self.assertNotIn("My name is Chen Junqi", "/".join(hints))
        self.assertTrue(all(float(hint.rsplit(":", 1)[1]) <= 1.2 for hint in hints))

    def test_invalid_values_and_non_english_are_empty(self):
        for value in [None, 4, {}, ["hello"], "", "我今天要去吃饭。", "25 100"]:
            self.assertEqual(practice_hotwords(value), "")

    def test_delimiters_scores_and_newlines_cannot_inject_context_commands(self):
        hints = practice_hotwords("hello / :9999\nworld :1000\nI'm fine")
        self.assertNotIn("9999", hints)
        self.assertNotIn("1000", hints)
        self.assertNotIn("\n", hints)
        self.assertIn("I'm :1.0", hints.split("/"))

    def test_context_is_bounded_and_duplicate_words_are_not_amplified(self):
        self.assertEqual(practice_hotwords("hello " * 3000), practice_hotwords("hello"))
        self.assertLessEqual(len(practice_hotwords(" ".join("Word" + chr(65+i%26) for i in range(2000)))), MAX_HOTWORDS)
        self.assertEqual(practice_hotwords("z" * 100), "")

    def test_only_practice_stream_receives_this_rounds_context(self):
        recognizer = Mock(practice_context_enabled=True)
        Recognition(recognizer, "Chen Junqi")
        self.assertIn("Chen Junqi :1.2", recognizer.create_stream.call_args.args[0])
        Recognition(recognizer)
        self.assertEqual(recognizer.create_stream.call_args.args, ())
        Recognition(recognizer, "Alice Smith")
        self.assertNotIn("Chen", recognizer.create_stream.call_args.args[0])

    def test_unsupported_context_or_stream_failure_falls_back(self):
        recognizer = Mock(practice_context_enabled=False)
        Recognition(recognizer, "Chen Junqi")
        recognizer.create_stream.assert_called_once_with()
        recognizer = Mock(practice_context_enabled=True)
        recognizer.create_stream.side_effect = [RuntimeError("context unavailable"), Mock()]
        Recognition(recognizer, "Chen Junqi")
        self.assertEqual(recognizer.create_stream.call_count, 2)
        self.assertEqual(recognizer.create_stream.call_args.args, ())


class VocabTests(unittest.TestCase):
    def test_chinese_hint_whitelist_uses_actual_prefixed_token_ids(self):
        with tempfile.TemporaryDirectory() as folder:
            model = Path(folder)
            self.assertEqual(chinese_context_tokens(model), frozenset())
            (model / "tokens.txt").write_text("<blk> 0\n▁我 1\n▁今 2\n▁天 3\n龘 4\n", encoding="utf-8")
            self.assertEqual(chinese_context_tokens(model), frozenset("我今天"))
    def test_unknown_or_missing_vocab_is_not_applied_to_a_different_model(self):
        with tempfile.TemporaryDirectory() as folder:
            model = Path(folder)
            self.assertIsNone(prepare_bpe_vocab(model))
            (model / "tokens.txt").write_text("<blk> 0\n▁hello 1\n", encoding="utf-8")
            self.assertIsNone(prepare_bpe_vocab(model))
            self.assertFalse((model / "bpe.vocab").exists())

    def test_known_vocab_is_atomic_reused_and_bootstraps_chinese(self):
        tokens = "<blk> 0\n▁hello 1\n▁你 2\n".encode("utf-8")
        with tempfile.TemporaryDirectory() as folder:
            model = Path(folder)
            (model / "tokens.txt").write_bytes(tokens)
            with patch("bpe_vocab.XASR_TOKENS_SHA256", hashlib.sha256(tokens).hexdigest()):
                target = prepare_bpe_vocab(model)
                self.assertEqual(target.read_text(encoding="utf-8"),
                                 "<blk>\t0\n▁hello\t-1\n▁你\t-2\n▁\t-1\n你\t-999999\n")
                modified = target.stat().st_mtime_ns
                self.assertEqual(prepare_bpe_vocab(model), target)
                self.assertEqual(target.stat().st_mtime_ns, modified)
                self.assertFalse(list(model.glob("*.tmp")))

    def test_model_creation_falls_back_when_context_cannot_load(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            model = root / "streaming"
            model.mkdir()
            for filename in ["encoder.onnx", "decoder.onnx", "joiner.onnx"]:
                (model / filename).touch()
            with patch("recognition.prepare_bpe_vocab", return_value=None), \
                    patch("recognition.sherpa_onnx.OnlineRecognizer.from_transducer") as factory:
                result = create_recognizer(root)
                self.assertFalse(result.practice_context_enabled)
                self.assertEqual(factory.call_args.kwargs["decoding_method"], "greedy_search")
            with patch("recognition.prepare_bpe_vocab", return_value=model / "bpe.vocab"), \
                    patch("recognition.sherpa_onnx.OnlineRecognizer.from_transducer",
                          side_effect=[RuntimeError("bad context"), Mock()]) as factory:
                self.assertFalse(create_recognizer(root).practice_context_enabled)
                self.assertEqual(factory.call_count, 2)
                self.assertEqual(factory.call_args.kwargs["decoding_method"], "greedy_search")
