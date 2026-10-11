"""普通中英混说的有界英文词汇提示；只影响声学解码，不替换转写结果。"""

import re


MAX_TEXT = 8192
MAX_WORDS = 96
MAX_HOTWORDS = 4096
WORD = re.compile(r"[A-Za-z]+(?:'[A-Za-z]+)?")


def limit_vocabulary(text):
    if not isinstance(text, str):
        return None
    if len(text) > MAX_TEXT:
        # 截断的半个单词不能作为新的词汇提示。
        text = re.sub(r"[A-Za-z’‘']+$", "", text[:MAX_TEXT])
    return text


def dictation_hotwords(text):
    """接收本轮个人词汇；弱提示保留中文候选，不把词表当作指定答案。"""
    text = limit_vocabulary(text)
    if not text:
        return ""
    text = text.replace("’", "'").replace("‘", "'")
    groups, seen = [], set()
    for match in WORD.finditer(text):
        word = match.group()
        key = word.lower()
        # 单字母容易对应中文音节，不给额外语言偏置。
        if not 2 <= len(word) <= 48 or key in seen:
            continue
        if len(seen) >= MAX_WORDS:
            break
        seen.add(key)
        # 每个词只弱加分；大小写变体不叠加到同一条候选路径。
        groups.append([f"{variant} :0.8" for variant in dict.fromkeys((word, key, word.capitalize()))])
    entries = [entry for group in groups for entry in group]
    if sum(len(entry) + 1 for entry in entries) <= MAX_HOTWORDS:
        return "/".join(entries)
    # 先让每个词至少保留原始形式，剩余空间才补大小写，防止前面的
    # 常见词变体挤掉后面已提交的个人专名。保留固定总预算，不增推理开销。
    entries, length = [], 0
    primary = [group[0] for group in groups]
    variants = [entry for group in groups for entry in group[1:]]
    for entry in primary + variants:
        if length + len(entry) + 1 <= MAX_HOTWORDS:
            entries.append(entry)
            length += len(entry) + 1
    return "/".join(entries)
