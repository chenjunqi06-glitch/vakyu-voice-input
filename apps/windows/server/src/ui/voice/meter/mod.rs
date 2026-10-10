//! 最近五个真实音量包络；不使用时间、随机数或固定动画生成高度。

#[cfg(test)]
mod tests;

#[derive(Default)]
pub(in crate::ui::voice) struct VoiceMeter {
    samples: [u8; 5],
}

impl VoiceMeter {
    pub fn push(&mut self, level: u8) -> bool {
        let previous = self.samples;
        if level == 0 {
            self.clear();
        } else {
            self.samples.rotate_left(1);
            self.samples[4] = level.min(100);
        }
        previous != self.samples
    }

    pub fn clear(&mut self) {
        self.samples = [0; 5];
    }

    pub fn quiet(&self) -> bool {
        self.samples == [0; 5]
    }

    pub fn heights(&self) -> [i32; 5] {
        self.samples.map(|level| 2 + i32::from(level) * 21 / 100)
    }
}
