//! 换轮、取消和高频替换只保留当前译文，不积压待翻译段落。

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use tokio::sync::oneshot;

use crate::voice_practice::completion::Completion;
use crate::voice_practice::translation::VoicePracticeTranslation;
use crate::voice_practice::{VoicePracticeError, VoicePracticeTranslator, worker};

#[test]
fn cancel_and_reused_public_request_id_cannot_deliver_old_generation() {
    let (mut translator, mut requests, responses) = VoicePracticeTranslator::channels();
    translator.request(42, "第一段").unwrap();
    let first = requests.borrow_and_update().clone().unwrap();
    translator.cancel();
    responses
        .send(Some(Completion {
            generation: first.generation,
            translation: VoicePracticeTranslation {
                request_id: 42,
                result: Ok("First.".to_owned()),
            },
        }))
        .unwrap();
    assert!(translator.try_recv().is_none());
    translator.request(42, "第二段").unwrap();
    assert!(translator.try_recv().is_none());
    let second = requests.borrow_and_update().clone().unwrap();
    responses
        .send(Some(Completion {
            generation: second.generation,
            translation: VoicePracticeTranslation {
                request_id: 42,
                result: Ok("Second.".to_owned()),
            },
        }))
        .unwrap();
    assert_eq!(translator.try_recv().unwrap().result.unwrap(), "Second.");
    assert!(translator.try_recv().is_none());
}

#[test]
fn invalid_sources_are_not_truncated_or_sent() {
    let (mut translator, requests, _responses) = VoicePracticeTranslator::channels();
    assert_eq!(
        translator.request(1, "  "),
        Err(VoicePracticeError::EmptySource)
    );
    assert_eq!(
        translator.request(2, &"字".repeat(4001)),
        Err(VoicePracticeError::SourceTooLong)
    );
    assert!(requests.borrow().is_none());
    translator.request(3, &"字".repeat(1055)).unwrap();
    assert_eq!(
        requests.borrow().as_ref().unwrap().source.chars().count(),
        1055
    );
}

#[tokio::test]
async fn thousands_of_replacements_keep_only_latest_without_starting_old_network_work() {
    let (mut translator, requests, responses) = VoicePracticeTranslator::channels();
    for id in 1..=10_000 {
        translator.request(id, "我今天要去吃饭。").unwrap();
    }
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    let task = tokio::spawn(worker::serve(requests, responses, move |_, _| {
        observed.fetch_add(1, Ordering::SeqCst);
        async { Ok("I am going out to eat today.".to_owned()) }
    }));
    tokio::task::yield_now().await;
    let result = translator.try_recv().unwrap();
    assert_eq!(result.request_id, 10_000);
    assert!(result.result.is_ok());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    drop(translator);
    task.await.unwrap();
}

#[tokio::test]
async fn cancel_drops_the_inflight_future_and_next_request_can_complete() {
    let (mut translator, requests, responses) = VoicePracticeTranslator::channels();
    let (started_tx, started_rx) = oneshot::channel();
    let (abandoned_tx, abandoned_rx) = oneshot::channel::<()>();
    let mut started_tx = Some(started_tx);
    let mut abandoned_rx = Some(abandoned_rx);
    let task = tokio::spawn(worker::serve(requests, responses, move |_, _| {
        let stalled = abandoned_rx.take();
        let started = started_tx.take();
        async move {
            if let Some(started) = started {
                let _ = started.send(());
            }
            if let Some(stalled) = stalled {
                let _ = stalled.await;
            }
            Ok("Ready.".to_owned())
        }
    }));
    translator.request(1, "第一段").unwrap();
    started_rx.await.unwrap();
    translator.cancel();
    tokio::task::yield_now().await;
    assert!(abandoned_tx.is_closed());
    assert!(translator.try_recv().is_none());
    translator.request(2, "第二段").unwrap();
    tokio::task::yield_now().await;
    assert_eq!(translator.try_recv().unwrap().request_id, 2);
    drop(translator);
    task.await.unwrap();
}
