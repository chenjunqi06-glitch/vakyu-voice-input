//! Alt 长按的纯状态机；短按与组合键不启动录音，松开只发送一次停止。

use std::time::{Duration, Instant};

const HOLD_DELAY: Duration = Duration::from_millis(180);

const FINAL_TIMEOUT: Duration = Duration::from_secs(12);

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub(super) enum Hold {
    #[default]
    Idle,
    Pending(Instant),
    Recording,
    Finishing(Instant),
    /// 后端达到时长上限已返回结果，但 Alt 尚未松开。
    HoldingFinal,
    /// 结果已到且 Alt 已松开，等当前拼音组句结束后插入。
    AwaitingCommit,
    /// 组合键打断后，必须等 Alt 松开才能再次启动。
    Blocked,
}

impl Hold {
    pub(super) fn arm(&mut self, now: Instant, repeat: bool, allowed: bool) {
        if *self == Self::Idle && !repeat && allowed {
            *self = Self::Pending(now);
        }
    }

    pub(super) fn ready(self, now: Instant) -> bool {
        matches!(self, Self::Pending(since) if now.duration_since(since) >= HOLD_DELAY)
    }

    /// 返回是否需要向 Server 发送停止；未启动的短按仍由应用处理。
    pub(super) fn release(&mut self, now: Instant) -> bool {
        match *self {
            Self::Recording => {
                *self = Self::Finishing(now);
                true
            }
            Self::Pending(_) | Self::Blocked => {
                *self = Self::Idle;
                false
            }
            Self::HoldingFinal => {
                *self = Self::AwaitingCommit;
                false
            }
            Self::Idle | Self::Finishing(_) | Self::AwaitingCommit => false,
        }
    }

    /// 新桥已报告 Press 时，即使尚未开始录音，松键也必须通知 Server 撤销长按。
    pub(super) fn release_control(&mut self, now: Instant) -> bool {
        if matches!(self, Self::Pending(_)) {
            *self = Self::Finishing(now);
            true
        } else {
            self.release(now)
        }
    }

    pub(super) fn recording_started(&mut self) {
        if matches!(self, Self::Pending(_)) {
            *self = Self::Recording;
        }
    }

    pub(super) fn interrupt(&mut self, alt_down: bool) -> bool {
        let cancel = self.server_active();
        *self = if alt_down { Self::Blocked } else { Self::Idle };
        cancel
    }

    pub(super) fn server_active(self) -> bool {
        matches!(self, Self::Recording | Self::Finishing(_))
    }

    /// 新桥轮询不拉键盘候选；松键后继续组句时，要让同一拍再收普通云结果。
    /// 旧协议已经借普通 Poll 收语音，本拍不能再重复轮询。
    pub(super) fn occupies_poll(self, bridge: bool, composing: bool, translating: bool) -> bool {
        match self {
            Self::Idle | Self::Blocked | Self::AwaitingCommit => false,
            Self::Finishing(_) if bridge && (composing || translating) => false,
            _ => true,
        }
    }

    pub(super) fn timed_out(self, now: Instant) -> bool {
        matches!(self, Self::Finishing(since) if now.duration_since(since) >= FINAL_TIMEOUT)
    }

    /// 松开后继续键入不取消结果；按住期间的第二个键仍表示 Alt 组合键。
    pub(super) fn cancel_on_key(self) -> bool {
        matches!(
            self,
            Self::Pending(_) | Self::Recording | Self::HoldingFinal
        )
    }

    pub(super) fn received_final(&mut self) -> bool {
        match self {
            Self::Pending(_) | Self::Recording => *self = Self::HoldingFinal,
            Self::Finishing(_) => *self = Self::AwaitingCommit,
            _ => return false,
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::{HOLD_DELAY, Hold};
    use std::time::{Duration, Instant};

    #[test]
    fn a_tap_and_repeats_do_not_start_recording() {
        let now = Instant::now();
        let mut hold = Hold::Idle;
        hold.arm(now, false, true);
        assert!(!hold.ready(now + Duration::from_millis(179)));
        assert!(!hold.release(now));
        hold.arm(now, true, true);
        assert_eq!(hold, Hold::Idle);
        hold.arm(now, false, false);
        assert_eq!(hold, Hold::Idle);
    }

    #[test]
    fn hold_starts_after_threshold_and_release_stops_only_once() {
        let now = Instant::now();
        let mut hold = Hold::Idle;
        hold.arm(now, false, true);
        assert!(hold.ready(now + HOLD_DELAY));
        hold = Hold::Recording;
        assert!(hold.release(now));
        assert!(!hold.release(now));
        assert!(hold.server_active());
        assert!(!hold.timed_out(now + Duration::from_secs(11)));
        assert!(hold.timed_out(now + Duration::from_secs(12)));
    }

    #[test]
    fn chord_cancels_and_cannot_rearm_until_alt_is_released() {
        let now = Instant::now();
        for initial in [Hold::Pending(now), Hold::Recording, Hold::Finishing(now)] {
            let mut hold = initial;
            assert_eq!(hold.interrupt(true), initial.server_active());
            hold.arm(now, false, true);
            assert_eq!(hold, Hold::Blocked);
            assert!(!hold.release(now));
            hold.arm(now, false, true);
            assert_eq!(hold, Hold::Pending(now));
        }
    }

    #[test]
    fn automatic_final_is_retained_until_alt_release() {
        let now = Instant::now();
        let mut hold = Hold::Recording;
        assert!(hold.received_final());
        assert_eq!(hold, Hold::HoldingFinal);
        assert!(!hold.release(now));
        assert_eq!(hold, Hold::AwaitingCommit);
        assert!(!hold.received_final());
        assert!(!hold.release(now));
    }

    #[test]
    fn typing_after_release_keeps_result_and_cached_final_never_times_out() {
        let now = Instant::now();
        let mut hold = Hold::Recording;
        assert!(hold.cancel_on_key());
        hold.release(now);
        assert!(!hold.cancel_on_key());
        assert!(hold.received_final());
        assert!(!hold.cancel_on_key());
        assert!(!hold.timed_out(now + Duration::from_secs(3600)));
    }

    #[test]
    fn bridge_release_before_long_hold_still_notifies_server_once() {
        let now = Instant::now();
        let mut hold = Hold::Pending(now);
        assert!(!hold.server_active(), "Armed不能让短按Alt被吃掉");
        assert!(hold.release_control(now));
        assert!(!hold.release_control(now));
        hold.recording_started();
        assert!(
            matches!(hold, Hold::Finishing(_)),
            "迟到Started不能重开录音"
        );
        assert!(hold.received_final());
        assert_eq!(hold, Hold::AwaitingCommit);
    }

    #[test]
    fn bridge_start_is_controlled_by_server_without_local_delay() {
        let now = Instant::now();
        let mut hold = Hold::Pending(now);
        assert!(!hold.ready(now));
        hold.recording_started();
        assert_eq!(hold, Hold::Recording);
        assert!(hold.release_control(now));
    }

    #[test]
    fn finishing_bridge_keeps_keyboard_and_translation_results_polling() {
        let finishing = Hold::Finishing(Instant::now());
        for (composing, translating) in [(true, false), (false, true), (true, true)] {
            assert!(
                !finishing.occupies_poll(true, composing, translating),
                "新桥停止录音后不能阻塞键盘云结果"
            );
            assert!(
                finishing.occupies_poll(false, composing, translating),
                "旧协议已调用普通Poll，不能同拍重复读取"
            );
        }
        assert!(finishing.occupies_poll(true, false, false));
    }

    #[test]
    fn holding_voice_owns_poll_but_inactive_and_cached_results_do_not() {
        let now = Instant::now();
        for bridge in [false, true] {
            for state in [Hold::Pending(now), Hold::Recording, Hold::HoldingFinal] {
                assert!(state.occupies_poll(bridge, true, true));
            }
            for state in [Hold::Idle, Hold::Blocked, Hold::AwaitingCommit] {
                assert!(!state.occupies_poll(bridge, true, true));
            }
        }
    }
}
