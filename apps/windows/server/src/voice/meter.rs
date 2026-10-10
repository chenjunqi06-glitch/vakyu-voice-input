//! 麦克风真实音量的瞬时快照；按录音代次隔离，旧样本过期后归零。

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const FRESH_FOR: Duration = Duration::from_millis(300);

#[derive(Clone, Default)]
pub(super) struct AudioMeter {
    sample: Arc<Mutex<Option<(u64, u8, Instant)>>>,
}

impl AudioMeter {
    pub fn update(&self, generation: u64, level: u8) {
        if let Ok(mut sample) = self.sample.lock() {
            *sample = Some((generation, level.min(100), Instant::now()));
        }
    }

    pub fn level(&self, generation: u64) -> u8 {
        self.level_at(generation, Instant::now())
    }

    fn level_at(&self, generation: u64, now: Instant) -> u8 {
        self.sample
            .lock()
            .ok()
            .and_then(|sample| *sample)
            .filter(|(current, _, at)| {
                *current == generation && now.saturating_duration_since(*at) < FRESH_FOR
            })
            .map_or(0, |(_, level, _)| level)
    }
}

#[cfg(test)]
mod tests {
    use super::{AudioMeter, FRESH_FOR};
    use std::time::Instant;

    #[test]
    fn actual_levels_are_bounded_and_stale_or_other_recordings_are_silent() {
        let meter = AudioMeter::default();
        assert_eq!(meter.level(1), 0);
        meter.update(1, 255);
        assert_eq!(meter.level(1), 100);
        assert_eq!(meter.level(2), 0);
        assert_eq!(meter.level_at(1, Instant::now() + FRESH_FOR), 0);
        meter.update(2, 41);
        assert_eq!(meter.level(2), 41);
        meter.update(2, 0);
        assert_eq!(meter.level(2), 0);
    }
}
