//! 后台线程：收请求、防抖、查缓存、发网络请求、回结果。

use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use qingjian_core::{Prediction, PredictionRequest};
use tokio::sync::mpsc::UnboundedReceiver;

use crate::prompt::Reply;

use crate::cache::PredictionCache;
use crate::chat_client::ChatClient;
use crate::error::PredictError;

/// 缓存条数。
const CACHE_CAPACITY: usize = 64;

pub struct Worker {
    /// 请求入口。主线程 drop 掉发送端后线程自然退出。
    requests: UnboundedReceiver<PredictionRequest>,

    /// 结果出口。
    responses: Sender<Prediction>,

    /// 网络客户端。
    client: ChatClient,

    /// 防抖窗口。
    debounce: Duration,

    /// 结果缓存。
    cache: PredictionCache,
}

/// 缓存里存的是解析后的回复。
type Cached = Reply;

impl Worker {
    pub fn new(
        requests: UnboundedReceiver<PredictionRequest>,
        responses: Sender<Prediction>,
        client: ChatClient,
        debounce: Duration,
    ) -> Self {
        Self {
            requests,
            responses,
            client,
            debounce,
            cache: PredictionCache::with_capacity(CACHE_CAPACITY),
        }
    }

    /// 阻塞运行直到发送端全部关闭。
    pub fn run(self) -> Result<(), PredictError> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        runtime.block_on(self.run_async())
    }

    async fn run_async(mut self) -> Result<(), PredictError> {
        let mut pending = None;
        loop {
            let request = match pending.take() {
                Some(request) => Some(request),
                None => self.requests.recv().await,
            };
            let Some(mut request) = request else {
                return Ok(());
            };
            // 队列里可能已有多次按键，只处理最新内容。
            while let Ok(newer) = self.requests.try_recv() {
                request = newer;
            }
            // 已有结果时立即返回，缓存命中不应该再等防抖窗口。
            let key = PredictionCache::key(&request);
            if let Some(reply) = self.cache.get(&key) {
                tracing::debug!(sequence = request.sequence, kind = ?request.kind, "联想命中缓存");
                self.reply(request.sequence, reply.clone());
                continue;
            }

            tokio::select! {
                biased;
                newer = self.requests.recv() => {
                    let Some(newer) = newer else { return Ok(()); };
                    pending = Some(newer);
                    continue;
                }
                _ = tokio::time::sleep(self.debounce) => {}
            };

            let start = Instant::now();
            // 已发给服务商的请求不中断；在途期间只记住最新输入，避免取消后重发造成重复计费。
            let (result, mut newest, mut latest_sequence) = {
                let completion = self.client.complete(&request);
                tokio::pin!(completion);
                let mut newest = None;
                let mut latest_sequence = request.sequence;
                let result = loop {
                    tokio::select! {
                        biased;
                        result = &mut completion => break result,
                        incoming = self.requests.recv() => {
                            let Some(incoming) = incoming else { return Ok(()); };
                            latest_sequence = self.coalesce_inflight(incoming, &mut newest);
                        }
                    }
                };
                (result, newest, latest_sequence)
            };
            // 请求完成与通道入队可能同时发生；以队列中最新内容为准。
            while let Ok(incoming) = self.requests.try_recv() {
                latest_sequence = self.coalesce_inflight(incoming, &mut newest);
            }
            match result {
                Ok(reply) => {
                    tracing::info!(
                        sequence = request.sequence,
                        elapsed_ms = start.elapsed().as_millis(),
                        words = reply.words.len(),
                        sentence = reply.sentence.is_some(),
                        english = reply.english_sentence.is_some(),
                        "联想完成"
                    );
                    // 空回复不进缓存：模型偶尔什么都不给（问字尤其），缓存住就等于这个问题以后永远没答案，
                    // 用户重打一遍也只会命中缓存（2026-09-07 `?mumumu` 就是这么卡住的）
                    if !reply.is_empty() {
                        self.cache.insert(key, reply.clone());
                    }
                    if latest_sequence == request.sequence {
                        self.reply(request.sequence, reply);
                    }
                }
                Err(error) => {
                    tracing::warn!(sequence = request.sequence, %error, "联想失败");
                    if latest_sequence == request.sequence {
                        // 清除当前输入的等待状态；后续新输入仍可继续查询。
                        self.reply(request.sequence, Reply::default());
                    }
                }
            }
            pending = newest;
        }
    }

    fn reply(&self, sequence: u64, reply: Cached) {
        // 接收端没了说明 Predictor 已经被 drop，线程随后也会退出
        let words = reply.words;
        let sentence = reply.sentence;
        let english = reply.english_sentence;
        let _ = self.responses.send(Prediction {
            sequence,
            words,
            sentence: match (sentence, english) {
                (Some(sentence), Some(english)) => {
                    Some(format!("{sentence}\u{1f}QJ_ENGLISH\u{1f}{english}"))
                }
                (Some(sentence), None) => Some(sentence),
                (None, Some(english)) => Some(format!("\u{1f}QJ_ENGLISH\u{1f}{english}")),
                (None, None) => None,
            },
        });
    }

    fn coalesce_inflight(
        &self,
        request: PredictionRequest,
        newest: &mut Option<PredictionRequest>,
    ) -> u64 {
        let sequence = request.sequence;
        let key = PredictionCache::key(&request);
        *newest = None;
        if let Some(reply) = self.cache.get(&key) {
            tracing::debug!(sequence, kind = ?request.kind, "联想命中缓存");
            self.reply(sequence, reply.clone());
        } else {
            *newest = Some(request);
        }
        sequence
    }
}
