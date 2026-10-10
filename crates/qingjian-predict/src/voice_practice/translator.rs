//! 平台调用的非阻塞入口：请求和结果都仅保留最新一项。

use qingjian_core::Language;
use tokio::sync::watch;

use super::completion::Completion;
use super::error::VoicePracticeError;
use super::prompt::MAX_SOURCE_CHARS;
use super::request::Request;
use super::translation::VoicePracticeTranslation;
use super::worker;
use crate::chat_client::ChatClient;
use crate::config::PredictConfig;

/// 跟读翻译与候选联想互相独立；调用方仅在用户开启跟读后构造本对象。
pub struct VoicePracticeTranslator {
    requests: watch::Sender<Option<Request>>,

    responses: watch::Receiver<Option<Completion>>,

    active: Option<(u64, u64)>,

    generation: u64,
}

impl VoicePracticeTranslator {
    /// 只复用云服务凭据和模型，不要求 `predict.enabled`，不读取光标上下文。
    pub fn new(config: &PredictConfig) -> Result<Self, VoicePracticeError> {
        let api_key = config
            .resolve_api_key()
            .ok_or(VoicePracticeError::MissingApiKey)?;
        let url = reqwest::Url::parse(config.base_url.trim())
            .map_err(|_| VoicePracticeError::InvalidEndpoint)?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err(VoicePracticeError::InvalidEndpoint);
        }
        if config.model.trim().is_empty() {
            return Err(VoicePracticeError::MissingModel);
        }
        // 候选词常用的 5 秒预算不足以翻译千字口述；整段翻译独立设置两分钟上限。
        let mut config = config.clone();
        config.base_url = config.base_url.trim().to_owned();
        config.model = config.model.trim().to_owned();
        config.timeout_ms = 120_000;
        let client = ChatClient::new(&config, api_key);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| VoicePracticeError::Unavailable)?;
        let (translator, requests, responses) = Self::channels();
        std::thread::Builder::new()
            .name("qingjian-voice-translation".to_owned())
            .spawn(move || runtime.block_on(worker::run(requests, responses, client)))
            .map_err(|_| VoicePracticeError::Unavailable)?;
        Ok(translator)
    }

    /// 兼容既有中文转英文调用；反向跟读使用 `request_to`。
    pub fn request(&mut self, request_id: u64, chinese: &str) -> Result<(), VoicePracticeError> {
        self.request_to(request_id, chinese, Language::English)
    }

    /// 新一轮连同目标语言替换旧一轮，取消旧网络 future；不会积压请求。
    pub fn request_to(
        &mut self,
        request_id: u64,
        source: &str,
        target: Language,
    ) -> Result<(), VoicePracticeError> {
        if !matches!(target, Language::English | Language::Chinese) {
            return Err(VoicePracticeError::UnsupportedLanguage);
        }
        let source = source.trim();
        if source.is_empty() {
            return Err(VoicePracticeError::EmptySource);
        }
        if source.chars().count() > MAX_SOURCE_CHARS {
            return Err(VoicePracticeError::SourceTooLong);
        }
        self.generation = self.generation.wrapping_add(1);
        self.requests
            .send(Some(Request {
                generation: self.generation,
                request_id,
                source: source.to_owned(),
                target,
            }))
            .map_err(|_| VoicePracticeError::Unavailable)?;
        self.active = Some((self.generation, request_id));
        Ok(())
    }

    /// 只交付当前轮次的新结果，取消后到达或被新一轮替换的结果一律丢弃。
    pub fn try_recv(&mut self) -> Option<VoicePracticeTranslation> {
        let (generation, request_id) = self.active?;
        match self.responses.has_changed() {
            Ok(false) => None,
            Ok(true) => {
                let completion = self.responses.borrow_and_update().clone()?;
                if completion.generation != generation {
                    return None;
                }
                self.active = None;
                Some(completion.translation)
            }
            Err(_) => {
                self.active = None;
                Some(VoicePracticeTranslation {
                    request_id,
                    result: Err(VoicePracticeError::Unavailable),
                })
            }
        }
    }

    pub fn cancel(&mut self) {
        self.active = None;
        let _ = self.requests.send(None);
    }

    pub(super) fn channels() -> (
        Self,
        watch::Receiver<Option<Request>>,
        watch::Sender<Option<Completion>>,
    ) {
        let (requests, request_rx) = watch::channel(None);
        let (response_tx, responses) = watch::channel(None);
        (
            Self {
                requests,
                responses,
                active: None,
                generation: 0,
            },
            request_rx,
            response_tx,
        )
    }
}
