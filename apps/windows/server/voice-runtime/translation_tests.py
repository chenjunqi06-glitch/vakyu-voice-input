"""离线翻译的切句、拼接、协议与失败处理；用假模型，不需要 CTranslate2。"""

import io
import json
import queue
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path

import translate_main
from translation import (
    OfflineTranslator,
    TranslationError,
    join_sentences,
    model_files_present,
    split_sentences,
)


class FakeModel:
    def __init__(self, path, prefix):
        self.path = path
        self.prefix = prefix
        self.calls = []

    def translate(self, sentences):
        self.calls.append(list(sentences))
        return [f"<{sentence}>" for sentence in sentences]


def model_root(directory):
    for folder in ("zh-en", "en-zh"):
        path = Path(directory) / folder
        path.mkdir(parents=True)
        for name in ("model.bin", "source.spm", "target.spm"):
            (path / name).write_bytes(b"x")
    return Path(directory)


class SplitTests(unittest.TestCase):
    def test_chinese_and_english_sentences(self):
        self.assertEqual(split_sentences("我饿了。我们去吃饭吧！", "zh"), ["我饿了。", "我们去吃饭吧！"])
        self.assertEqual(
            split_sentences("I am hungry. Shall we eat? Yes", "en"),
            ["I am hungry.", "Shall we eat?", "Yes"],
        )
        self.assertEqual(split_sentences("   ", "en"), [])

    def test_long_sentence_is_cut_at_commas(self):
        line = "，".join(["今天天气很好"] * 60) + "。"
        pieces = split_sentences(line, "zh")
        self.assertGreater(len(pieces), 1)
        self.assertEqual("".join(pieces), line)

    def test_join_uses_spaces_only_for_english(self):
        self.assertEqual(join_sentences(["A.", "B."], "en"), "A. B.")
        self.assertEqual(join_sentences(["甲。", "乙。"], "zh"), "甲。乙。")


class TranslatorTests(unittest.TestCase):
    def test_lines_are_kept_and_models_load_once_per_direction(self):
        with tempfile.TemporaryDirectory() as directory:
            root = model_root(directory)
            self.assertTrue(model_files_present(root))
            translator = OfflineTranslator(root, factory=FakeModel)
            result = translator.translate("你好。\n\n再见！", "en")
            self.assertEqual(result, "<你好。>\n\n<再见！>")
            translator.translate("谢谢", "en")
            self.assertEqual(len(translator.models), 1)
            chinese = translator.translate("Hello", "zh")
            self.assertEqual(chinese, "<Hello>")
            self.assertEqual(translator.models["zh"].prefix, ">>cmn_Hans<<")
            self.assertIsNone(translator.models["en"].prefix)

    def test_missing_models_and_bad_targets_are_readable_errors(self):
        with tempfile.TemporaryDirectory() as directory:
            translator = OfflineTranslator(Path(directory), factory=FakeModel)
            self.assertFalse(model_files_present(directory))
            with self.assertRaisesRegex(TranslationError, "重新安装"):
                translator.translate("你好", "en")
            with self.assertRaises(TranslationError):
                translator.translate("你好", "ja")


class ProtocolTests(unittest.TestCase):
    def run_lines(self, translator, commands):
        lines = queue.Queue()
        for command in commands:
            lines.put(command)
        lines.put(None)
        output = io.StringIO()
        with redirect_stdout(output):
            translate_main.run(translator, lines, idle_seconds=1)
        return [json.loads(line) for line in output.getvalue().splitlines()]

    def test_results_carry_ids_and_failures_do_not_stop_the_loop(self):
        with tempfile.TemporaryDirectory() as directory:
            translator = OfflineTranslator(model_root(directory), factory=FakeModel)
            events = self.run_lines(
                translator,
                [
                    "not json\n",
                    json.dumps({"command": "translate", "id": 1, "text": "你好", "target": "ja"}),
                    json.dumps({"command": "translate", "id": 2, "text": "你好", "target": "en"}),
                    json.dumps({"command": "shutdown"}),
                    json.dumps({"command": "translate", "id": 3, "text": "不会处理", "target": "en"}),
                ],
            )
        self.assertEqual([event["id"] for event in events], [1, 2])
        self.assertFalse(events[0]["ok"])
        self.assertTrue(events[1]["ok"])
        self.assertEqual(events[1]["text"], "<你好>")

    def test_idle_timeout_exits(self):
        lines = queue.Queue()
        translate_main.run(None, lines, idle_seconds=0.05)


if __name__ == "__main__":
    unittest.main()
