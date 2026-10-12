//! User-adjustable ASR similarity thresholds.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PracticeMatchConfig {
    /// Minimum edit-distance similarity for ordinary words, in percent.
    pub word_similarity: u8,

    /// More permissive threshold for names confirmed by the Chinese source.
    pub name_similarity: u8,
}

impl Default for PracticeMatchConfig {
    fn default() -> Self {
        Self {
            word_similarity: 78,
            name_similarity: 45,
        }
    }
}

impl PracticeMatchConfig {
    pub fn normalized(self) -> Self {
        Self {
            word_similarity: self.word_similarity.min(100),
            name_similarity: self.name_similarity.min(100),
        }
    }
}
