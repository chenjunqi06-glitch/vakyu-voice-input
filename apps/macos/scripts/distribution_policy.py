"""源码与安装包共用的个人资料排除规则；只报告路径，不输出疑似凭据。"""

from pathlib import PurePosixPath
import hashlib
import re

PRIVATE_SUFFIXES = frozenset({".wav", ".mp3", ".m4a", ".aac", ".flac", ".ogg", ".pcm",
                              ".aiff", ".caf", ".p12", ".pfx", ".key", ".log"})
FORBIDDEN_PARTS = frozenset({".git", "__pycache__", ".venv", "venv", "node_modules",
                            "site-packages-cache", ".pytest_cache", ".build", "recordings"})
SECRET_PATTERNS = (
    re.compile(rb"\bsk-[A-Za-z0-9_-]{20,}"),
    re.compile(rb"\bgh[pousr]_[A-Za-z0-9]{30,}"),
    re.compile(rb"-----BEGIN (?:RSA |EC |OPENSSH |DSA )?PRIVATE KEY-----"),
)
# 原有日志脱敏测试中的两个固定假凭据，仅允许原文件与精确指纹，不能豁免新增值。
TEST_FIXTURES = {
    "crates/qingjian-platform/src/logs/secrets/mod.rs": frozenset({
        "c871f067542d565c57ebb8b54f99afe326644dceeb5c05fc2209a8161a52875e",
        "cb61e720e8558e3f8d4c7ccf512e4adc7d2e0692117c0eb911ab649aeb0872ea",
    })
}


def private_path(relative, source=False):
    path = PurePosixPath(relative)
    name = path.name.lower()
    parts = tuple(part.lower() for part in path.parts)
    if name.startswith(".env") or any(part in FORBIDDEN_PARTS for part in parts):
        return True
    if source and parts and parts[0].startswith("target"):
        return True
    if name == "config.toml" and not (source and relative == ".cargo/config.toml"):
        return True
    if path.suffix.lower() in PRIVATE_SUFFIXES:
        return True
    if name.startswith("user") and path.suffix.lower() in (".tsv", ".json", ".qj", ".db", ".sqlite"):
        return True
    if name in {"english-practice-progress.json", "chinese-practice-progress.json",
                "voice-practice-progress.json", "credentials", "credentials.json",
                "credentials.toml", "secrets.json", "secrets.toml"}:
        return True
    if "practice" in name and "progress" in name and path.suffix.lower() == ".json":
        return True
    return False


def check_credentials(content, relative):
    allowed = TEST_FIXTURES.get(relative, ())
    for pattern in SECRET_PATTERNS:
        for match in pattern.finditer(content):
            if hashlib.sha256(match.group()).hexdigest() not in allowed:
                raise ValueError(f"发现疑似真实凭据，已停止交付：{relative}")
