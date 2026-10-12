//! 合并 TSF 事件与轮询的 Alt 边沿；切到应用时已经按住的键不能当成新长按。

#[derive(Default)]
pub(super) struct Press {
    down: Option<bool>,

    foreground: bool,
}

impl Press {
    /// 轮询先看见松开，之后才接受新按下，避免 Alt+Tab 切入时误开麦。
    pub(super) fn sample(&mut self, down: bool, foreground: bool) -> bool {
        let pressed = self.down == Some(false) && down && self.foreground && foreground;
        self.down = Some(down);
        self.foreground = foreground;
        pressed
    }

    /// 明确的非重复按键事件可作为首个边沿；OnTest / OnKey 共用这一份状态。
    pub(super) fn key_down(&mut self, repeat: bool) -> bool {
        let pressed = self.down != Some(true) && !repeat;
        self.down = Some(true);
        pressed
    }

    pub(super) fn release(&mut self) {
        self.down = Some(false);
    }

    pub(super) fn interrupt(&mut self, down: bool) {
        self.down = Some(down);
        self.foreground = false;
    }
}

#[cfg(test)]
mod tests {
    use super::Press;
    use crate::com::service::voice::hold::Hold;
    use crate::com::service::voice::keys::AltKeys;
    use std::time::{Duration, Instant};

    #[test]
    fn polling_starts_a_hold_without_any_tsf_alt_events() {
        let mut press = Press::default();
        let mut hold = Hold::Idle;
        let now = Instant::now();
        assert!(!press.sample(false, true));
        if press.sample(true, true) {
            hold.arm(now, false, true);
        }
        assert!(hold.ready(now + Duration::from_millis(200)));
        assert!(!press.sample(true, true));
    }

    #[test]
    fn test_key_key_down_and_poll_capture_only_once() {
        let mut press = Press::default();
        assert!(press.key_down(false));
        assert!(!press.key_down(false));
        assert!(!press.sample(true, true));
        assert!(!press.key_down(true));
        press.release();
        assert!(press.key_down(false));
    }

    #[test]
    fn polling_before_tsf_event_does_not_capture_twice() {
        let mut press = Press::default();
        press.sample(false, true);
        assert!(press.sample(true, true));
        assert!(!press.key_down(false));
    }

    #[test]
    fn activation_or_focus_gain_with_alt_already_held_waits_for_release() {
        let mut press = Press::default();
        assert!(!press.sample(true, true));
        assert!(!press.key_down(true));
        assert!(!press.sample(true, true));
        assert!(!press.sample(false, true));
        assert!(press.sample(true, true));

        // 后台线程也持续采样；切回前台不产生第二个按下边沿。
        assert!(!press.sample(true, true));

        // Alt+Tab 在两拍间切入：上拍在后台，即使此前见过 Alt 松开也不能启动。
        press.sample(false, false);
        assert!(!press.sample(true, true));
        assert!(!press.sample(true, true));
        press.sample(false, true);
        assert!(press.sample(true, true));
    }

    #[test]
    fn releasing_the_other_chord_key_does_not_restart_voice() {
        let now = Instant::now();
        let mut press = Press::default();
        let mut hold = Hold::Idle;
        press.sample(false, true);
        assert!(press.sample(true, true));
        hold.arm(now, false, true);
        hold.interrupt(true);
        assert_eq!(hold, Hold::Blocked);
        assert!(!press.sample(true, true));
        assert!(!press.key_down(true));
        assert!(!press.sample(false, true));
        hold.release(now);
        assert!(press.sample(true, true));
        hold.arm(now, false, true);
        assert!(hold.ready(now + Duration::from_millis(200)));
    }

    #[test]
    fn ineligible_press_is_not_retried_when_context_changes() {
        let mut press = Press::default();
        press.sample(false, true);
        assert!(press.sample(true, true));
        // 焦点不符、私密框、组句中或禁用配置使该边沿被拒绝；恢复后仍须松开再按。
        assert!(!press.sample(true, true));
        assert!(!press.key_down(false));
        press.sample(false, true);
        assert!(press.sample(true, true));
    }

    #[test]
    fn right_alt_without_generic_state_starts_and_releasing_left_keeps_recording() {
        let mut press = Press::default();
        let mut keys = AltKeys::default();
        let mut hold = Hold::Idle;
        let now = Instant::now();
        assert!(!press.sample(keys.down(), true));
        keys.right = true;
        assert!(press.sample(keys.down(), true));
        hold.arm(now, false, true);
        assert!(hold.ready(now + Duration::from_millis(200)));
        hold = Hold::Recording;

        keys.left = true;
        assert!(!press.sample(keys.down(), true));
        keys.left = false;
        assert!(keys.down());
        assert!(!press.sample(keys.down(), true));
        assert_eq!(hold, Hold::Recording);
        keys.right = false;
        assert!(!keys.down());
        assert!(!press.sample(keys.down(), true));
        assert!(hold.release(now));
    }

    #[test]
    fn left_alt_and_generic_driver_state_each_support_fresh_presses() {
        let mut press = Press::default();
        for keys in [
            AltKeys {
                left: true,
                ..Default::default()
            },
            AltKeys {
                generic: true,
                ..Default::default()
            },
        ] {
            press.sample(false, true);
            assert!(press.sample(keys.down(), true));
            press.sample(false, true);
        }
    }
}
