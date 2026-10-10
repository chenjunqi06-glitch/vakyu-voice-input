"""长口述的文本累计回归；不加载模型，不访问麦克风。"""

import unittest
from unittest.mock import Mock, patch

import numpy as np

from recognition import Recognition, join_segments


class RecognitionTests(unittest.TestCase):
    def setUp(self):
        self.recognizer = Mock()
        self.recognizer.is_ready.return_value = False
        self.recognizer.is_endpoint.return_value = False
        self.recognizer.get_result.return_value = ""
        self.session = Recognition(self.recognizer)
        self.second = np.zeros(16000, dtype=np.float32)

    def accept(self, text, endpoint=False):
        self.recognizer.get_result.return_value = text
        self.recognizer.is_endpoint.return_value = endpoint
        return self.session.accept(16000, self.second)

    def test_over_thousand_characters_survive_segments_and_long_pauses(self):
        paragraphs = [
            "今天我们要讨论的事情比较多，请先把已经确认的时间地点和安排完整记录下来，后面我还会继续补充。",
            "明天下午我们先去办公室整理文件，然后联系负责运输的同事，确认每一件东西都放在正确的位置。",
            "如果中间有需要核对的内容，我会停下来思考一会儿，等我接着说的时候前面的文字也应该保留下来。",
        ] * 10
        expected = ""
        audio_seconds = 0
        for paragraph in paragraphs:
            for offset in range(0, len(paragraph), 5):
                partial = paragraph[:offset + 5]
                self.assertEqual(self.accept(partial), expected + partial)
                audio_seconds += 1
            expected += paragraph
            self.assertEqual(self.accept(paragraph, endpoint=True), expected)
            audio_seconds += 1
            # 停顿再触发空段 endpoint 也不能撤销上一段。
            for _ in range(20):
                self.assertEqual(self.accept("", endpoint=True), expected)
                audio_seconds += 1
        self.assertGreater(len(expected), 1000)
        self.assertGreater(audio_seconds, 300)
        self.recognizer.get_result.return_value = "最后这句话也要保留。"
        self.assertEqual(self.session.finish(), expected + "最后这句话也要保留。")
        self.recognizer.create_stream.return_value.input_finished.assert_called_once()

    def test_revised_current_segment_does_not_duplicate_completed_text(self):
        self.accept("已经确认的第一段。", endpoint=True)
        self.assertEqual(self.accept("下一段暂时错误"), "已经确认的第一段。下一段暂时错误")
        self.assertEqual(self.accept("下一段修正文字"), "已经确认的第一段。下一段修正文字")
        self.assertEqual(self.accept(""), "已经确认的第一段。")

    def test_empty_audio_blocks_reuse_accumulated_transcript(self):
        self.accept("完成的一段。", endpoint=True)
        self.accept("还在继续说话")
        with patch("recognition.join_segments", wraps=join_segments) as join:
            for _ in range(500):
                self.assertEqual(self.accept("还在继续说话"), "完成的一段。还在继续说话")
            join.assert_not_called()

    def test_release_after_endpoint_preserves_completed_text(self):
        self.accept("hello", endpoint=True)
        self.accept("world", endpoint=True)
        self.recognizer.get_result.return_value = ""
        self.assertEqual(self.session.finish(), "hello world")

    def test_both_languages_preserve_repeated_words_and_foreign_names(self):
        for language, first, second, expected in [
            ("zh", "我叫陈君琪。", "我叫陈君琪，我们去吃饭。", "我叫陈君琪。我叫陈君琪，我们去吃饭。"),
            ("en", "My name is", "Chen Junqi. I I want lunch.", "My name is Chen Junqi. I I want lunch."),
        ]:
            self.session = Recognition(self.recognizer, language=language)
            self.accept(first, endpoint=True)
            self.accept("", endpoint=True)
            self.assertEqual(self.accept(second), expected)
            self.recognizer.get_result.return_value = second
            self.assertEqual(self.session.finish(), expected)


if __name__ == "__main__":
    unittest.main()
