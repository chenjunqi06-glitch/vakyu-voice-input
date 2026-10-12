//! 开麦前后核对原子请求；冷初始化期间松键或取消时，不能随后补开麦克风。

use qingjian_platform::protocol::VoiceUpdate;
use std::sync::atomic::{AtomicU64, Ordering};

/// 高位是代次，最低位是是否仍按住；一次原子读取不会混用两个时刻的状态。
pub(super) fn requested(desired: &AtomicU64, generation: u64) -> Option<bool> {
    let value = desired.load(Ordering::Acquire);
    (value >> 1 == generation).then_some(value & 1 != 0)
}

pub(super) fn prepare_and_start<T>(
    desired: &AtomicU64,
    generation: u64,
    prepare: impl FnOnce() -> Result<T, String>,
    start: impl FnOnce(&T) -> Result<(), String>,
    cancel: impl FnOnce(&T),
) -> Result<T, Option<VoiceUpdate>> {
    check_held(desired, generation)?;
    let backend = prepare().map_err(|message| failure(desired, generation, message))?;
    // prepare 包含 COM、识别模型与听写加载，可能慢于一次完整的按下/松开。
    check_held(desired, generation)?;
    start(&backend).map_err(|message| failure(desired, generation, message))?;
    // 处理恰好在开麦调用中松键的边界；立即关闭，不让迟到的 Started 重开界面。
    if let Err(update) = check_held(desired, generation) {
        cancel(&backend);
        return Err(update);
    }
    Ok(backend)
}

fn check_held(desired: &AtomicU64, generation: u64) -> Result<(), Option<VoiceUpdate>> {
    match requested(desired, generation) {
        Some(true) => Ok(()),
        Some(false) => Err(Some(VoiceUpdate::Final {
            text: String::new(),
        })),
        None => Err(None),
    }
}

fn failure(desired: &AtomicU64, generation: u64, message: String) -> Option<VoiceUpdate> {
    match check_held(desired, generation) {
        Ok(()) => Some(VoiceUpdate::Error { message }),
        Err(update) => update,
    }
}

#[cfg(test)]
mod tests {
    use super::{prepare_and_start, requested};
    use qingjian_platform::protocol::VoiceUpdate;
    use std::cell::Cell;
    use std::sync::atomic::{AtomicU64, Ordering};

    #[test]
    fn release_during_cold_initialization_never_opens_audio() {
        let desired = AtomicU64::new(9);
        let opened = Cell::new(false);
        let result = prepare_and_start(
            &desired,
            4,
            || {
                desired.store(8, Ordering::Release);
                Ok(())
            },
            |_| {
                opened.set(true);
                Ok(())
            },
            |_| {},
        );
        assert_eq!(
            result,
            Err(Some(VoiceUpdate::Final {
                text: String::new()
            }))
        );
        assert!(!opened.get());
    }

    #[test]
    fn cancelled_initialization_cannot_open_audio_for_a_new_request() {
        let desired = AtomicU64::new(9);
        let opened = Cell::new(false);
        let result = prepare_and_start(
            &desired,
            4,
            || {
                desired.store(11, Ordering::Release);
                Ok(())
            },
            |_| {
                opened.set(true);
                Ok(())
            },
            |_| {},
        );
        assert_eq!(result, Err(None));
        assert!(!opened.get());
        assert_eq!(requested(&desired, 5), Some(true));
    }

    #[test]
    fn release_during_audio_start_closes_it_without_reporting_started() {
        let desired = AtomicU64::new(9);
        let closed = Cell::new(false);
        let result = prepare_and_start(
            &desired,
            4,
            || Ok(()),
            |_| {
                desired.store(8, Ordering::Release);
                Ok(())
            },
            |_| closed.set(true),
        );
        assert_eq!(
            result,
            Err(Some(VoiceUpdate::Final {
                text: String::new()
            }))
        );
        assert!(closed.get());
    }

    #[test]
    fn already_released_request_does_not_even_load_the_model() {
        let desired = AtomicU64::new(8);
        let result = prepare_and_start(
            &desired,
            4,
            || -> Result<(), String> {
                panic!("过期请求不能初始化识别器");
            },
            |_| Ok(()),
            |_| {},
        );
        assert_eq!(
            result,
            Err(Some(VoiceUpdate::Final {
                text: String::new()
            }))
        );
    }
}
