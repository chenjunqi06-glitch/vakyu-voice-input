use std::collections::HashSet;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use qingjian_core::{FilledGloss, Language};

use super::prompt;
use crate::chat_client::ChatClient;
use crate::error::PredictError;

/// 收到第一个词后最多等这么久再发，攒同一批。
pub const BATCH_WAIT: Duration = Duration::from_millis(1500);

/// 一批最多几个词。
pub const BATCH_SIZE: usize = 8;

/// 回复的 token 上限：8 个词各两条短译词绰绰有余。
const MAX_TOKENS: u32 = 600;

/// 后台线程：攒词、发请求、回释义。
pub struct GlossWorker {
    /// 请求入口。主线程 drop 掉发送端后线程自然退出。
    requests: Receiver<(Language, String)>,

    /// 结果出口。
    responses: Sender<FilledGloss>,

    /// 网络客户端。
    client: ChatClient,

    /// 已成功解析出释义的 (语言, 词)；失败或无效回复仍可重试。
    resolved: HashSet<(Language, String)>,
}

impl GlossWorker {
    pub fn new(
        requests: Receiver<(Language, String)>,
        responses: Sender<FilledGloss>,
        client: ChatClient,
    ) -> Self {
        Self {
            requests,
            responses,
            client,
            resolved: HashSet::new(),
        }
    }

    /// 阻塞运行直到发送端全部关闭。
    pub fn run(mut self) -> Result<(), PredictError> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        while let Ok(first) = self.requests.recv() {
            let Some(batch) = self.collect(first) else {
                return Ok(());
            };
            // 一批里可能混着两种语言（中途切了学习语言）：按语言各发一次
            let mut languages: Vec<Language> = Vec::new();
            for (language, _) in &batch {
                if !languages.contains(language) {
                    languages.push(*language);
                }
            }
            for language in languages {
                let words: Vec<String> = batch
                    .iter()
                    .filter(|(l, _)| *l == language)
                    .map(|(_, w)| w.clone())
                    .collect();
                self.ask(&runtime, language, words);
            }
        }
        Ok(())
    }

    /// 从第一个词起攒一批：等到 [`BATCH_WAIT`] 或攒够 [`BATCH_SIZE`]；已成功的词跳过。发送端关闭返回 `None`。
    fn collect(&mut self, first: (Language, String)) -> Option<Vec<(Language, String)>> {
        let deadline = Instant::now() + BATCH_WAIT;
        let mut batch = Vec::with_capacity(BATCH_SIZE);
        self.take(&mut batch, first);
        while batch.len() < BATCH_SIZE {
            let remaining = deadline.saturating_duration_since(Instant::now());
            match self.requests.recv_timeout(remaining) {
                Ok(item) => self.take(&mut batch, item),
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => return None,
            }
        }
        Some(batch)
    }

    fn take(&mut self, batch: &mut Vec<(Language, String)>, item: (Language, String)) {
        if !self.resolved.contains(&item) && !batch.contains(&item) {
            batch.push(item);
        }
    }

    fn ask(&mut self, runtime: &tokio::runtime::Runtime, language: Language, words: Vec<String>) {
        if words.is_empty() {
            return;
        }
        let start = Instant::now();
        let system = prompt::system_prompt(language);
        let user = prompt::user_prompt(&words);
        match runtime.block_on(self.client.chat(system, &user, MAX_TOKENS)) {
            Ok(content) => {
                let filled = prompt::parse_reply(&content, language, &words);
                self.resolved
                    .extend(filled.iter().map(|gloss| (language, gloss.word.clone())));
                tracing::info!(
                    language = language.code(),
                    asked = words.len(),
                    filled = filled.len(),
                    elapsed_ms = start.elapsed().as_millis(),
                    "释义兜底完成"
                );
                for gloss in filled {
                    // 接收端没了说明 Filler 已经被 drop，线程随后也会退出
                    if self.responses.send(gloss).is_err() {
                        return;
                    }
                }
            }
            Err(error) => {
                tracing::warn!(language = language.code(), asked = words.len(), %error, "释义兜底失败");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::thread;

    use crate::config::PredictConfig;

    use super::*;

    #[test]
    fn invalid_lookup_can_be_retried_until_a_gloss_is_resolved() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base_url = format!("http://{}/v1", listener.local_addr().unwrap());
        let responses = [
            ("200 OK", "not json"),
            (
                "200 OK",
                r#"{"items":[{"w":"测试","pos":"n.","senses":[{"t":"test"}]}]}"#,
            ),
        ];
        let server = thread::spawn(move || {
            for (status, content) in responses {
                let (mut stream, _) = listener.accept().unwrap();
                let mut reader = BufReader::new(&stream);
                let mut content_length = 0;
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if line == "\r\n" || line.is_empty() {
                        break;
                    }
                    if let Some((name, value)) = line.split_once(':')
                        && name.eq_ignore_ascii_case("content-length")
                    {
                        content_length = value.trim().parse::<usize>().unwrap();
                    }
                }
                reader.read_exact(&mut vec![0_u8; content_length]).unwrap();
                let body = if status.starts_with("200") {
                    serde_json::json!({
                        "id": "gloss-retry-test", "object": "chat.completion", "created": 0,
                        "model": "test-model", "choices": [{
                            "index": 0, "finish_reason": "stop",
                            "message": { "role": "assistant", "content": content }
                        }]
                    })
                    .to_string()
                } else {
                    content.to_owned()
                };
                let header = format!(
                    "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                    body.len()
                );
                stream.write_all(header.as_bytes()).unwrap();
                stream.write_all(body.as_bytes()).unwrap();
            }
        });

        let client = ChatClient::new(
            &PredictConfig {
                base_url,
                timeout_ms: 5000,
                ..PredictConfig::default()
            },
            "test-only-placeholder".to_owned(),
        );
        let (request_tx, request_rx) = mpsc::channel();
        drop(request_tx);
        let (response_tx, response_rx) = mpsc::channel();
        let mut worker = GlossWorker::new(request_rx, response_tx, client);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let item = (Language::English, "测试".to_owned());

        let mut batch = Vec::new();
        worker.take(&mut batch, item.clone());
        assert_eq!(batch, vec![item.clone()]);
        worker.ask(
            &runtime,
            item.0,
            batch.into_iter().map(|(_, word)| word).collect(),
        );
        assert!(response_rx.try_recv().is_err());

        let mut retry = Vec::new();
        worker.take(&mut retry, item.clone());
        assert_eq!(retry, vec![item.clone()]);
        worker.ask(
            &runtime,
            item.0,
            retry.into_iter().map(|(_, word)| word).collect(),
        );
        let filled = response_rx.try_recv().unwrap();
        assert_eq!(filled.word, item.1);
        assert_eq!(filled.translation.senses()[0].text, "test");

        let mut resolved = Vec::new();
        worker.take(&mut resolved, item);
        assert!(resolved.is_empty());
        server.join().unwrap();
    }
}
