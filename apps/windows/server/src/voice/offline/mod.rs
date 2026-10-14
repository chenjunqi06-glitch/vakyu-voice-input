//! 本机离线翻译：随包 Python 运行 `app/translate_main.py`（CTranslate2 + OPUS-MT），不联网、不花钱。
//! 进程由 [`engine`] 全局共享；每个 [`OfflineTranslator`] 只保留最新一轮请求，旧结果按编号丢弃。

mod engine;
mod pending;
mod reply;
mod request;

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};

use qingjian_core::Language;
use qingjian_predict::{VoicePracticeError, tidy_translation};

use self::pending::Pending;
use self::reply::Reply;

/// 与云端翻译相同的原文上限。
const MAX_SOURCE_CHARS: usize = 4000;

/// 离线翻译需要的文件；识别组件完整但缺这些时，只是不能离线翻译，不影响语音输入。
const REQUIRED_FILES: &[&str] = &[
    "python/pythonw.exe",
    "python/Lib/site-packages/ctranslate2/__init__.py",
    "python/Lib/site-packages/sentencepiece/__init__.py",
    "app/translate_main.py",
    "app/translation.py",
    "models/translate/zh-en/model.bin",
    "models/translate/zh-en/source.spm",
    "models/translate/zh-en/target.spm",
    "models/translate/en-zh/model.bin",
    "models/translate/en-zh/source.spm",
    "models/translate/en-zh/target.spm",
];

pub(crate) struct OfflineTranslator {
    root: PathBuf,

    /// 划词（保留换行）还是跟读（整理成一行）。
    reading: bool,

    replies: Sender<Reply>,

    results: Receiver<Reply>,

    active: Option<Pending>,
}

impl OfflineTranslator {
    /// 离线组件不完整时返回给用户看的原因。
    pub(crate) fn new(reading: bool) -> Result<Self, String> {
        let root = super::neural::runtime_root()
            .filter(|root| available(root))
            .ok_or_else(|| {
                "离线翻译模型没有安装完整，请重新安装 Vakyu 完整安装包；也可以在「云服务」里填写 API 密钥改用云端翻译"
                    .to_owned()
            })?;
        let (replies, results) = mpsc::channel();
        Ok(Self {
            root,
            reading,
            replies,
            results,
            active: None,
        })
    }

    /// 新一轮替换旧一轮；离线进程逐条处理，旧轮次的结果到达后丢弃。
    pub(crate) fn request(
        &mut self,
        request_id: u64,
        source: &str,
        target: Language,
    ) -> Result<(), String> {
        if !matches!(target, Language::English | Language::Chinese) {
            return Err(VoicePracticeError::UnsupportedLanguage
                .user_message()
                .to_owned());
        }
        let source = source.trim();
        if source.is_empty() {
            return Err(VoicePracticeError::EmptySource.user_message().to_owned());
        }
        if source.chars().count() > MAX_SOURCE_CHARS {
            return Err(VoicePracticeError::SourceTooLong.user_message().to_owned());
        }
        self.active = None;
        let wire = engine::submit(&self.root, source, target, self.replies.clone())?;
        self.active = Some(Pending {
            wire,
            request_id,
            target,
        });
        Ok(())
    }

    /// 只交付当前轮次；译文经 Core 的同一套校验整理。
    pub(crate) fn try_recv(&mut self) -> Option<(u64, Result<String, String>)> {
        let pending = self.active?;
        loop {
            match self.results.try_recv() {
                Ok(reply) if reply.wire == pending.wire => {
                    self.active = None;
                    let result = reply.result.and_then(|text| {
                        tidy_translation(&text, pending.target, self.reading)
                            .map_err(|error| error.user_message().to_owned())
                    });
                    return Some((pending.request_id, result));
                }
                Ok(_) => continue,
                Err(TryRecvError::Empty) => return None,
                Err(TryRecvError::Disconnected) => {
                    self.active = None;
                    return Some((
                        pending.request_id,
                        Err("离线翻译暂时不可用，请重试".to_owned()),
                    ));
                }
            }
        }
    }

    pub(crate) fn cancel(&mut self) {
        self.active = None;
    }
}

/// 离线翻译组件是否齐全；只检查非空文件，不启动进程。
pub(crate) fn available(root: &Path) -> bool {
    REQUIRED_FILES.iter().all(|file| {
        root.join(file)
            .metadata()
            .is_ok_and(|meta| meta.is_file() && meta.len() > 0)
    })
}

#[cfg(test)]
mod tests {
    use super::{REQUIRED_FILES, available};

    #[test]
    fn missing_any_model_file_disables_offline_translation() {
        let root = std::env::temp_dir().join(format!("vakyu-offline-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for file in REQUIRED_FILES {
            let path = root.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, b"x").unwrap();
        }
        assert!(available(&root));
        std::fs::remove_file(root.join("models/translate/en-zh/model.bin")).unwrap();
        assert!(!available(&root));
        let _ = std::fs::remove_dir_all(&root);
    }
}
