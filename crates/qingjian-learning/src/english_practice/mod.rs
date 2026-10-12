//! 英语跟读的纯文本进度计算；每次用完整识别结果重算，避免临时结果回改后误放行。

mod alignment;
mod assessment;
mod latin;
mod level;
mod match_config;
mod matcher;
mod names;
mod normalize;
mod number;
mod phonetic;
mod profile;
mod progress;
mod recovery;
mod round;
mod similarity;
mod step;
mod tolerance;

pub use assessment::PracticeAssessment;
pub use level::PracticeLevel;
pub use match_config::PracticeMatchConfig;
pub use matcher::EnglishPractice;
pub use profile::PracticeProfile;
pub use progress::PracticeProgress;
pub use round::PracticeRound;
pub use similarity::similarity_percent;

pub fn english_ipa_variants(word: &str) -> Option<Vec<String>> {
    phonetic::ipa_variants(word)
}

#[cfg(test)]
mod tests;
