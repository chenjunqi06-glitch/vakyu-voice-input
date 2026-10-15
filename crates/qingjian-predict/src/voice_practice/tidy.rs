//! 译文的统一校验与整理：云端回复与本机离线引擎的输出走同一套规则。

use qingjian_core::Language;

use super::error::VoicePracticeError;
use super::prompt::{MAX_REPLY_CHARS, valid_target_script};

/// 校验译文是目标语言、没有控制字符且不超长；跟读把空白压成一行，阅读（划词）逐行整理但保留换行。
pub fn tidy_translation(
    translation: &str,
    target: Language,
    reading: bool,
) -> Result<String, VoicePracticeError> {
    if !matches!(target, Language::English | Language::Chinese) {
        return Err(VoicePracticeError::UnsupportedLanguage);
    }
    if translation.chars().count() > MAX_REPLY_CHARS
        || translation
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\r' | '\n' | '\t'))
        || !valid_target_script(translation, target)
    {
        return Err(VoicePracticeError::InvalidTranslation);
    }
    if !reading {
        return Ok(translation.split_whitespace().collect::<Vec<_>>().join(" "));
    }
    let lines: Vec<String> = translation
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect();
    let tidied = lines.join("\n").trim_matches('\n').to_owned();
    if tidied.trim().is_empty() {
        return Err(VoicePracticeError::InvalidTranslation);
    }
    Ok(tidied)
}
