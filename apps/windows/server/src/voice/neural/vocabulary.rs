//! 普通口述的有界英文提示；仅在 helper 写管道线程读取本机已提交词表，不访问录音或云端。

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::command::HelperCommand;

pub(super) const MAX_WORDS: usize = 96;
pub(super) const MAX_BYTES: usize = 4096;
pub(super) const MAX_PERSONAL_WORDS: usize = 24;
pub(super) const MAX_CONFIG_BYTES: usize = 128 * 1024;
pub(super) const MAX_PERSONAL_BYTES: usize = 256 * 1024;

// 这组日常英文词已用中英混读音频比较验证；保持弱提示，不随意扩大到通用大词表。
pub(super) const BASE_WORDS: &str = "file folder email message meeting deadline update feedback project report document download upload link website browser account password login app application Windows macOS iPhone Android Google Apple GitHub ChatGPT Codex Vakyu API AI PDF Word Excel PowerPoint bug code model server client database token prompt test version feature online offline English hello thanks sorry please okay music video photo coffee lunch dinner office team schedule plan design review support";

type FileStamp = (u64, SystemTime);

/// 配置与词表按元数据缓存；每轮开始先检查开关，停用后不继续沿用个人提示。
pub(super) struct DictationVocabulary {
    user_dir: Option<PathBuf>,

    config_cache: Option<(Option<FileStamp>, bool)>,

    personal_cache: Option<(Option<FileStamp>, Vec<String>)>,
}

impl DictationVocabulary {
    pub fn new(user_dir: Option<PathBuf>) -> Self {
        Self {
            user_dir,
            config_cache: None,
            personal_cache: None,
        }
    }

    /// 停止、取消、退出及跟读必须省略字段，即使传入了上一轮的提示也先清除。
    pub fn apply(&mut self, command: &mut HelperCommand) {
        command.dictation_vocabulary = (command.command == "start"
            && command.practice_text.is_none())
        .then(|| self.vocabulary());
    }

    fn vocabulary(&mut self) -> String {
        let personal = if let Some(dir) = self.user_dir.clone() {
            if self.learning_enabled(&dir.join("config.toml")) {
                self.personal_words(&dir.join("user-english.tsv"))
            } else {
                self.personal_cache = None;
                Vec::new()
            }
        } else {
            Vec::new()
        };
        let mut result = String::new();
        let mut seen = HashSet::new();
        for word in BASE_WORDS
            .split_whitespace()
            .chain(personal.iter().map(String::as_str))
        {
            if !seen.insert(word.to_ascii_lowercase()) {
                continue;
            }
            if seen.len() > MAX_WORDS
                || result.len() + usize::from(!result.is_empty()) + word.len() > MAX_BYTES
            {
                break;
            }
            if !result.is_empty() {
                result.push(' ');
            }
            result.push_str(word);
        }
        result
    }

    fn learning_enabled(&mut self, path: &Path) -> bool {
        let Ok(stamp) = file_stamp(path) else {
            self.config_cache = None;
            return false;
        };
        if let Some((cached, enabled)) = self.config_cache
            && cached == stamp
        {
            return enabled;
        }
        // 不调用完整 Config::load，避免解析无关密钥、环境变量或记录私有配置。
        let enabled = if stamp.is_none() {
            true
        } else {
            read_bounded(path, MAX_CONFIG_BYTES)
                .ok()
                .and_then(|source| configured_learning(&source))
                .unwrap_or(false)
        };
        self.config_cache = Some((stamp, enabled));
        enabled
    }

    fn personal_words(&mut self, path: &Path) -> Vec<String> {
        let Ok(stamp) = file_stamp(path) else {
            self.personal_cache = None;
            return Vec::new();
        };
        if let Some((cached, words)) = &self.personal_cache
            && *cached == stamp
        {
            return words.clone();
        }
        let words = if stamp.is_none() {
            Vec::new()
        } else {
            read_bounded(path, MAX_PERSONAL_BYTES)
                .ok()
                .and_then(|source| personal_vocabulary(&source))
                .unwrap_or_default()
        };
        self.personal_cache = Some((stamp, words.clone()));
        words
    }
}

fn file_stamp(path: &Path) -> io::Result<Option<FileStamp>> {
    match path.metadata() {
        Ok(metadata) if metadata.is_file() => Ok(Some((metadata.len(), metadata.modified()?))),
        Ok(_) => Err(io::ErrorKind::InvalidData.into()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn read_bounded(path: &Path, maximum: usize) -> io::Result<String> {
    let file = File::open(path)?;
    if file.metadata()?.len() > maximum as u64 {
        return Err(io::ErrorKind::InvalidData.into());
    }
    let mut bytes = Vec::new();
    file.take(maximum as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > maximum {
        return Err(io::ErrorKind::InvalidData.into());
    }
    String::from_utf8(bytes).map_err(|_| io::ErrorKind::InvalidData.into())
}

fn configured_learning(source: &str) -> Option<bool> {
    let document = source.parse::<toml_edit::DocumentMut>().ok()?;
    let Some(general) = document.get("general") else {
        return Some(true);
    };
    match general.as_table_like()?.get("learning") {
        Some(value) => value.as_bool(),
        None => Some(true),
    }
}

/// 只保留可作为英文热词的已提交词；损坏计数或编码不猜测修复。
fn personal_vocabulary(source: &str) -> Option<Vec<String>> {
    let mut counted: HashMap<String, (String, u64)> = HashMap::new();
    for line in source.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut fields = line.split('\t');
        let word = fields.next()?;
        let count = fields.next()?.trim().parse::<u64>().ok()?;
        if fields.next().is_some() {
            return None;
        }
        if count == 0 || !valid_word(word) {
            continue;
        }
        let entry = counted
            .entry(word.to_ascii_lowercase())
            .or_insert_with(|| (word.to_owned(), 0));
        entry.1 = entry.1.saturating_add(count);
    }
    let base: HashSet<String> = BASE_WORDS
        .split_whitespace()
        .map(str::to_ascii_lowercase)
        .collect();
    let mut words: Vec<_> = counted
        .into_iter()
        .filter(|(key, _)| !base.contains(key))
        .collect();
    words.sort_unstable_by(|a, b| b.1.1.cmp(&a.1.1).then_with(|| a.0.cmp(&b.0)));
    Some(
        words
            .into_iter()
            .take(MAX_PERSONAL_WORDS)
            .map(|(_, (word, _))| word)
            .collect(),
    )
}

fn valid_word(word: &str) -> bool {
    (2..=48).contains(&word.len())
        && word
            .bytes()
            .all(|byte| byte.is_ascii_alphabetic() || byte == b'\'')
        && word.as_bytes()[0].is_ascii_alphabetic()
        && word.as_bytes()[word.len() - 1].is_ascii_alphabetic()
        && word.bytes().filter(|byte| *byte == b'\'').count() <= 1
}
