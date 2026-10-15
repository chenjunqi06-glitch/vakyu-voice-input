//! 模拟宿主在语音写入期间重入，重复轮询与重复编辑回调都不能再次写入。

use super::Submission;

#[test]
fn reentrant_poll_and_callback_cannot_insert_the_sentence_twice() {
    let state = Submission::default();
    let mut inserts = 0;
    assert!(state.queue());
    assert!(state.begin());
    assert!(state.queued());
    // 对应 SetSelection / InsertTextAtSelection 期间宿主再次轮询。
    assert!(!state.queue());
    assert!(!state.begin());
    inserts += 1;
    state.commit();
    state.finish();
    assert!(state.committed());
    assert!(!state.queue());
    assert!(!state.begin());
    assert_eq!(inserts, 1);
}

#[test]
fn composition_deferral_can_be_queued_after_callback_returns() {
    let state = Submission::default();
    assert!(state.queue());
    assert!(state.begin());
    assert!(!state.queue());
    state.finish();
    assert!(!state.queued());
    assert!(state.queue());
    assert!(state.begin());
    state.commit();
    state.finish();
    assert!(state.committed());
    assert!(!state.queue());
}

#[test]
fn failed_edit_request_releases_the_queue_gate() {
    let state = Submission::default();
    assert!(state.queue());
    state.finish();
    assert!(!state.queued());
    assert!(state.queue());
}
