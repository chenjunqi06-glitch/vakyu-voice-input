//! 候选帧缓存失效：页边界上的高亮变化、放行按键清除提示后都必须显示当前状态。

use qingjian_core::{CustomPhrase, Engine};
use qingjian_dictionary::Dictionary;
use qingjian_platform::protocol::{
    ClientMessage, Frame, KeyEvent, KeyModifiers, KeyOutcome, PROTOCOL_VERSION, ScreenRect,
    ServerMessage, SessionId,
};

use crate::dispatch::{Router, RouterConfig};

fn cached_router(highlight: usize, notice: Option<&str>) -> Router {
    let mut engine = Engine::new(Dictionary::parse("你\tni\t100\n").unwrap());
    engine
        .set_custom_phrases(
            (1..=9)
                .map(|position| CustomPhrase {
                    code: "qq".into(),
                    text: format!("第{position}项"),
                    position,
                    enabled: true,
                })
                .collect(),
        )
        .unwrap();
    let mut router = Router::new(
        engine,
        RouterConfig {
            page_size: 5,
            ..RouterConfig::default()
        },
    );
    router.handle(ClientMessage::OpenSession {
        session: SessionId(1),
        app: None,
        protocol: PROTOCOL_VERSION,
    });
    for character in "qq".chars() {
        key(
            &mut router,
            KeyEvent::new(character as u32, Some(character), KeyModifiers::default()),
        );
    }
    router.move_highlight(highlight as isize);
    router.notice = notice.map(str::to_owned);
    router.last_rect = Some(ScreenRect {
        left: 20,
        top: 20,
        right: 40,
        bottom: 40,
    });
    let frame = router.self_drawn_frame();
    router.reconcile_candidates(&frame);
    assert!(router.last_shown.is_some());
    router
}

fn key(router: &mut Router, event: KeyEvent) -> (KeyOutcome, Option<String>, Frame) {
    match router
        .handle(ClientMessage::Key {
            session: SessionId(1),
            event,
        })
        .unwrap()
    {
        ServerMessage::KeyResult {
            outcome,
            commit,
            frame,
            ..
        } => (outcome, commit, frame),
        other => panic!("expected KeyResult, got {other:?}"),
    }
}

#[test]
fn paging_at_an_edge_updates_cached_highlight_and_space_commit() {
    for (highlight, key_code, expected_highlight, expected_text) in
        [(2, 0x21, 0, "第1项"), (7, 0x22, 5, "第6项")]
    {
        let mut router = cached_router(highlight, None);
        let (_, _, shown) = key(
            &mut router,
            KeyEvent::new(key_code, None, KeyModifiers::default()),
        );
        assert_eq!(router.highlight, expected_highlight);
        assert_eq!(shown.page, expected_highlight / 5);
        assert_eq!(shown.highlight, 0, "页未变化时也要更新自绘缓存高亮");
        assert_eq!(router.last_shown.as_ref().unwrap().0.highlight, 0);
        assert_eq!(shown.candidates.items[shown.highlight].text, expected_text);
        let (_, committed, _) = key(
            &mut router,
            KeyEvent::new(0x20, Some(' '), KeyModifiers::default()),
        );
        assert_eq!(committed.as_deref(), Some(expected_text));
    }
}

#[test]
fn passthrough_key_clears_notice_in_the_cached_frame() {
    let mut router = cached_router(0, Some("已删除示例词"));
    assert!(router.last_shown.as_ref().unwrap().0.notice.is_some());
    let (outcome, committed, shown) = key(
        &mut router,
        KeyEvent::new(
            b'X' as u32,
            Some('x'),
            KeyModifiers {
                ctrl: true,
                ..KeyModifiers::default()
            },
        ),
    );
    assert_eq!(outcome, KeyOutcome::Passthrough);
    assert!(committed.is_none());
    assert!(router.notice.is_none());
    assert!(shown.notice.is_none());
    assert!(router.last_shown.as_ref().unwrap().0.notice.is_none());
    assert_eq!(shown.candidates.items[0].text, "第1项");
}
