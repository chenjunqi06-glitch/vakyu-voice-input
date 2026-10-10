"""把本轮中英文转为有界的弱热词提示，不把整句作为识别答案。"""

import re


MAX_TEXT = 8192
MAX_WORDS = 96
MAX_PHRASES = 12
MAX_HOTWORDS = 4096
WORD = re.compile(r"[A-Za-z]+(?:'[A-Za-z]+)?")
CJK = re.compile(r"[\u3400-\u4dbf\u4e00-\u9fff]+")


def practice_hotwords(text, language="en", chinese_tokens=None):
    if not isinstance(text, str):
        return ""
    text = text[:MAX_TEXT].replace("’", "'").replace("‘", "'")
    matches = list(WORD.finditer(text))[:512]
    entries, seen, unique = [], set(), set()

    def append(phrase, score):
        if phrase in seen:
            return
        item = f"{phrase} :{score}"
        if sum(len(entry) + 1 for entry in entries) + len(item) > MAX_HOTWORDS:
            return
        seen.add(phrase)
        entries.append(item)

    for match in matches:
        word = match.group()
        if len(word) > 48 or word.lower() in unique:
            continue
        if len(unique) >= MAX_WORDS:
            break
        unique.add(word.lower())
        # X-ASR 区分英文 token 大小写，常见首字母大写也需要提示。
        for variant in dict.fromkeys((word, word.lower(), word[:1].upper() + word[1:].lower())):
            append(variant, "1.0")

    phrases = 0
    if language == "zh":
        for match in CJK.finditer(text):
            phrase = match.group()
            for word in phrase:
                if (word in unique or len(unique) >= MAX_WORDS
                        or chinese_tokens is not None and word not in chinese_tokens):
                    continue
                unique.add(word)
                append(word, "0.6")
            for index in range(len(phrase) - 1):
                if phrases >= MAX_PHRASES:
                    break
                characters = phrase[index:index + 2]
                if chinese_tokens is not None and any(char not in chinese_tokens for char in characters):
                    continue
                # X-ASR 的每个汉字都带 ▁ 前缀；Sherpa 按空格逐词编码，
                # 双字不分隔会产生无 ID 的裸字，使整组提示失效。
                pair = " ".join(characters)
                if pair not in seen:
                    append(pair, "1.0")
                    phrases += 1
    for left, right in zip(matches, matches[1:]):
        first, second = left.group(), right.group()
        if (phrases >= MAX_PHRASES or not text[left.end():right.start()].isspace()
                or not first.istitle() or not second.istitle()
                or len(first) < 2 or len(second) < 2
                or len(first) > 48 or len(second) > 48):
            continue
        # 只给短姓名/专名搭配少量额外提示；不拼装用户的完整目标句。
        append(f"{first} {second}", "1.2")
        phrases += 1
    return "/".join(entries)
