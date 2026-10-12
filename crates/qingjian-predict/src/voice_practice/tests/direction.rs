//! 目标语言随代次一起替换，取消或切换方向不能交付前一轮译文。

use qingjian_core::Language;
use tokio::sync::oneshot;

use crate::voice_practice::completion::Completion;
use crate::voice_practice::translation::VoicePracticeTranslation;
use crate::voice_practice::{VoicePracticeError, VoicePracticeTranslator, worker};

#[test]
fn default_request_is_english_and_unsupported_target_keeps_current_request() {
    let (mut translator, mut requests, _responses) = VoicePracticeTranslator::channels();
    translator.request(4, "你好").unwrap();
    let first = requests.borrow_and_update().clone().unwrap();
    assert_eq!(first.target, Language::English);
    assert_eq!(first.source, "你好");
    for target in [Language::Japanese, Language::Spanish] {
        assert_eq!(
            translator.request_to(5, "Hello", target),
            Err(VoicePracticeError::UnsupportedLanguage)
        );
    }
    assert!(!requests.has_changed().unwrap());
    assert_eq!(
        requests.borrow().as_ref().unwrap().generation,
        first.generation
    );
}

#[test]
fn switching_target_with_reused_id_cannot_deliver_previous_language() {
    let (mut translator, mut requests, responses) = VoicePracticeTranslator::channels();
    translator.request(42, "你好").unwrap();
    let first = requests.borrow_and_update().clone().unwrap();
    translator
        .request_to(42, "Hello", Language::Chinese)
        .unwrap();
    let second = requests.borrow_and_update().clone().unwrap();
    assert_eq!(second.target, Language::Chinese);
    assert_ne!(first.generation, second.generation);
    responses
        .send(Some(Completion {
            generation: first.generation,
            translation: VoicePracticeTranslation {
                request_id: 42,
                result: Ok("Hello".to_owned()),
            },
        }))
        .unwrap();
    assert!(translator.try_recv().is_none());
    responses
        .send(Some(Completion {
            generation: second.generation,
            translation: VoicePracticeTranslation {
                request_id: 42,
                result: Ok("你好".to_owned()),
            },
        }))
        .unwrap();
    assert_eq!(translator.try_recv().unwrap().result.unwrap(), "你好");
}

#[tokio::test]
async fn canceled_translation_is_dropped_before_reverse_direction_is_delivered() {
    let (mut translator, requests, responses) = VoicePracticeTranslator::channels();
    let (started_tx, started_rx) = oneshot::channel();
    let (abandoned_tx, abandoned_rx) = oneshot::channel::<()>();
    let (reversed_tx, reversed_rx) = oneshot::channel();
    let mut started_tx = Some(started_tx);
    let mut abandoned_rx = Some(abandoned_rx);
    let mut reversed_tx = Some(reversed_tx);
    let task = tokio::spawn(worker::serve(requests, responses, move |source, target| {
        let stalled = abandoned_rx.take();
        let started = started_tx.take();
        let reversed = if stalled.is_none() {
            reversed_tx.take()
        } else {
            None
        };
        async move {
            if let Some(started) = started {
                assert_eq!(target, Language::English);
                assert_eq!(source, "你好");
                let _ = started.send(());
            }
            if let Some(stalled) = stalled {
                let _ = stalled.await;
                return Ok("Hello".to_owned());
            }
            assert_eq!(target, Language::Chinese);
            assert_eq!(source, "Hello");
            let _ = reversed.unwrap().send(());
            Ok("你好".to_owned())
        }
    }));
    translator.request(42, "你好").unwrap();
    started_rx.await.unwrap();
    translator.cancel();
    translator
        .request_to(42, "Hello", Language::Chinese)
        .unwrap();
    reversed_rx.await.unwrap();
    assert!(abandoned_tx.is_closed());
    assert_eq!(translator.try_recv().unwrap().result.unwrap(), "你好");
    assert!(translator.try_recv().is_none());
    drop(translator);
    task.await.unwrap();
}
