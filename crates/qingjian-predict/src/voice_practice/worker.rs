//! 单个网络 future 随请求替换或取消一起销毁，不阻塞语音事件线程。

use std::future::Future;

use qingjian_core::Language;
use tokio::sync::watch;

use super::completion::Completion;
use super::error::VoicePracticeError;
use super::prompt;
use super::reading;
use super::request::Request;
use super::translation::VoicePracticeTranslation;
use crate::chat_client::ChatClient;

pub(super) async fn run(
    requests: watch::Receiver<Option<Request>>,
    responses: watch::Sender<Option<Completion>>,
    client: ChatClient,
    reading: bool,
) {
    serve(requests, responses, |source, target| {
        let client = &client;
        async move {
            let system = if reading {
                reading::system_prompt(target)?
            } else {
                prompt::system_prompt(target)?
            };
            let content = client
                .chat_private(
                    system,
                    &prompt::user_prompt(&source, target)?,
                    prompt::MAX_TOKENS,
                )
                .await
                .map_err(VoicePracticeError::from)?;
            if reading {
                reading::parse_reply(&content, target)
            } else {
                prompt::parse_reply(&content, target)
            }
        }
    })
    .await;
}

pub(super) async fn serve<F, Fut>(
    mut requests: watch::Receiver<Option<Request>>,
    responses: watch::Sender<Option<Completion>>,
    mut translate: F,
) where
    F: FnMut(String, Language) -> Fut,
    Fut: Future<Output = Result<String, VoicePracticeError>>,
{
    loop {
        let current = requests.borrow_and_update().clone();
        let Some(request) = current else {
            if requests.changed().await.is_err() {
                return;
            }
            continue;
        };
        tokio::select! {
            biased;
            changed = requests.changed() => {
                if changed.is_err() {
                    return;
                }
            }
            result = translate(request.source, request.target) => {
                let completion = Completion {
                    generation: request.generation,
                    translation: VoicePracticeTranslation {
                        request_id: request.request_id,
                        result,
                    },
                };
                if responses.send(Some(completion)).is_err()
                    || requests.changed().await.is_err()
                {
                    return;
                }
            }
        }
    }
}
