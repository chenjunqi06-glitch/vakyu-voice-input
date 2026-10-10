//! 四个跟读操作共用绘制与命中尺寸；覆盖 540 DIP 窗口及高 DPI 边界。

use super::sample_view;
use crate::dispatch::PracticeAction;
use crate::ui::voice::practice::{action_at, action_bounds};

#[test]
fn practice_buttons_hit_only_their_drawn_regions() {
    let mut view = sample_view();
    view.actions_enabled = true;
    let size = (540, 600);
    assert_eq!(
        action_at(&view, 30, 570, size, 96),
        Some(PracticeAction::Retry)
    );
    assert_eq!(
        action_at(&view, 200, 570, size, 96),
        Some(PracticeAction::SkipCurrent)
    );
    assert_eq!(
        action_at(&view, 300, 570, size, 96),
        Some(PracticeAction::AcceptCurrent)
    );
    assert_eq!(
        action_at(&view, 450, 570, size, 96),
        Some(PracticeAction::Restart)
    );
    assert_eq!(action_at(&view, 144, 570, size, 96), None);
    assert_eq!(action_at(&view, 30, 500, size, 96), None);
}

#[test]
fn four_buttons_keep_their_bounds_and_gaps_at_high_dpi() {
    let mut view = sample_view();
    view.actions_enabled = true;
    let actions = [
        PracticeAction::Retry,
        PracticeAction::SkipCurrent,
        PracticeAction::AcceptCurrent,
        PracticeAction::Restart,
    ];
    for dpi in [96, 120, 144, 192] {
        let scale = |value| value * dpi as i32 / 96;
        let size = (scale(540), scale(600));
        let bounds = action_bounds(size.0, size.1, dpi);
        assert_eq!(bounds[0].left, scale(22));
        assert_eq!(bounds[3].right, size.0 - scale(22));
        for (index, rect) in bounds.iter().enumerate() {
            assert!(rect.right - rect.left >= scale(116));
            assert_eq!(rect.top, size.1 - scale(44));
            assert_eq!(rect.bottom - rect.top, scale(30));
            assert_eq!(
                action_at(&view, rect.left, rect.top, size, dpi),
                Some(actions[index])
            );
            assert_eq!(
                action_at(&view, rect.right - 1, rect.bottom - 1, size, dpi),
                Some(actions[index])
            );
            assert_eq!(action_at(&view, rect.right, rect.top, size, dpi), None);
            if index + 1 < bounds.len() {
                assert_eq!(bounds[index + 1].left - rect.right, scale(8));
            }
        }
    }
    view.actions_enabled = false;
    assert_eq!(action_at(&view, 450, 570, (540, 600), 96), None);
}
