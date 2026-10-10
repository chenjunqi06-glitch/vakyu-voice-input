//! 识别消费边界随累计文本修订而移动；保留最后一次实际得分的快照防止重复计分。

#[derive(Default)]
pub(super) struct RecognitionEvidence {
    units: Vec<String>,

    consumed: usize,
}

impl RecognitionEvidence {
    pub fn consumed_in(&self, revised: &[String]) -> usize {
        let prefix = self
            .units
            .iter()
            .zip(revised)
            .take_while(|(left, right)| left == right)
            .count();
        if self.consumed <= prefix {
            return self.consumed;
        }
        let suffix = self.units[prefix..]
            .iter()
            .rev()
            .zip(revised[prefix..].iter().rev())
            .take_while(|(left, right)| left == right)
            .count();
        let old_end = self.units.len() - suffix;
        let new_end = revised.len() - suffix;
        if self.consumed > old_end || (self.consumed == old_end && suffix > 0) {
            new_end + (self.consumed - old_end)
        } else {
            prefix + (self.consumed - prefix).min(new_end - prefix)
        }
    }

    pub fn accept(&mut self, units: Vec<String>, consumed: usize) {
        self.consumed = consumed.min(units.len());
        self.units = units;
    }

    pub fn clear(&mut self) {
        self.units.clear();
        self.consumed = 0;
    }
}
