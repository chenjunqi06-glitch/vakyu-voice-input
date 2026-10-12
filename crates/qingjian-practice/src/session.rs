//! 单轮跟读正文和识别进度，仅存在内存；收尾被修正时可继续补读。

use std::time::{Duration, Instant};

use qingjian_learning::{PracticeLevel, PracticeMatchConfig, PracticeProgress};

use crate::{PracticeView, PracticeWordCard};

use super::matcher::PracticeMatcher;
use super::phase::Phase;

pub struct PracticeSession {
    original: String,

    translated_text: String,

    matcher: Option<PracticeMatcher>,

    learn_english: bool,

    pronunciations: Vec<String>,

    level: PracticeLevel,

    level_progress: String,

    match_config: PracticeMatchConfig,

    demo_unavailable: bool,

    pub phase: Phase,

    confirmed: String,

    current: String,

    matched: usize,

    stable_since: Option<Instant>,

    phase_since: Instant,

    error: String,

    feedback: String,

    word_card: Option<PracticeWordCard>,
}

impl PracticeSession {
    pub fn new(original: String, level: PracticeLevel, level_progress: String) -> Self {
        Self::new_with_match_config(
            original,
            level,
            level_progress,
            PracticeMatchConfig::default(),
        )
    }

    pub fn new_with_match_config(
        original: String,
        level: PracticeLevel,
        level_progress: String,
        match_config: PracticeMatchConfig,
    ) -> Self {
        Self::new_for_language(original, level, level_progress, match_config, true)
    }

    pub fn new_for_language(
        original: String,
        level: PracticeLevel,
        level_progress: String,
        match_config: PracticeMatchConfig,
        learn_english: bool,
    ) -> Self {
        Self {
            original,
            translated_text: String::new(),
            matcher: None,
            learn_english,
            pronunciations: Vec::new(),
            level,
            level_progress,
            match_config: match_config.normalized(),
            demo_unavailable: false,
            phase: Phase::Translating,
            confirmed: String::new(),
            current: String::new(),
            matched: 0,
            stable_since: None,
            phase_since: Instant::now(),
            error: String::new(),
            feedback: String::new(),
            word_card: None,
        }
    }

    pub fn translated(&mut self, english: &str) -> bool {
        if self.phase != Phase::Translating {
            return false;
        }
        let matcher = PracticeMatcher::new(
            english,
            &self.original,
            self.level,
            self.match_config,
            self.learn_english,
        );
        if !matcher.is_valid() {
            self.fail("没有可跟读的目标语言译文，请取消后重试".to_owned());
            return false;
        }
        if !self.learn_english {
            self.pronunciations = qingjian_translate::chinese_practice_readings(matcher.words());
        }
        self.translated_text = english.to_owned();
        self.matcher = Some(matcher);
        self.current.clear();
        self.feedback.clear();
        self.phase = Phase::Preparing;
        self.phase_since = Instant::now();
        true
    }

    pub fn demonstrating(&mut self) {
        if self.phase == Phase::Preparing {
            self.phase = Phase::Demonstrating;
            self.phase_since = Instant::now();
        }
    }

    pub fn demo_finished(&mut self, unavailable: bool) {
        if self.phase == Phase::Demonstrating {
            self.demo_unavailable = unavailable;
            self.phase = Phase::Preparing;
            self.phase_since = Instant::now();
        }
    }

    pub fn demo_rate(&self) -> i32 {
        match self.level {
            PracticeLevel::Newcomer => -2,
            PracticeLevel::Intermediate => -1,
            PracticeLevel::Challenge => 0,
        }
    }

    /// 只提示本轮目标译句，不携带原话或其他应用上下文；中文保持连续，供本机双字提示使用。
    pub fn recognition_hint(&self) -> String {
        self.translated_text.clone()
    }

    pub fn progress(&self) -> PracticeProgress {
        PracticeProgress {
            matched_words: self.matched,
            total_words: self
                .matcher
                .as_ref()
                .map_or(0, |matcher| matcher.words().len()),
            complete: self.phase == Phase::Complete,
        }
    }

    pub fn started(&mut self) {
        if self.phase == Phase::Preparing {
            self.phase = Phase::Reading;
            self.phase_since = Instant::now();
        }
    }

    pub fn interim(&mut self, text: &str, now: Instant) -> bool {
        // VoiceManager 合并同一拍的更新时可能以首个 Interim 覆盖 Started。
        let was_preparing = self.phase == Phase::Preparing;
        if was_preparing {
            self.started();
        }
        if self.phase != Phase::Reading {
            return false;
        }
        let text_changed = self.current != text;
        if text_changed {
            self.current = text.to_owned();
            self.stable_since = None;
        }
        let Some(matcher) = &self.matcher else {
            return false;
        };
        let progress = matcher.evaluate(&self.transcript(text));
        let changed = self.matched != progress.matched_words;
        self.matched = progress.matched_words;
        if progress.complete {
            self.stable_since.get_or_insert(now);
        } else {
            self.stable_since = None;
        }
        was_preparing || changed || text_changed
    }

    pub fn should_check(&self, now: Instant) -> bool {
        self.phase == Phase::Reading
            && self.stable_since.is_some_and(|since| {
                // A natural pause between English words should not force an
                // extra round of recording; only commit after a clear, longer
                // period of stable complete recognition.
                // ASR interim events are generally emitted every few hundred
                // milliseconds. Waiting 1.2 s made the release gate miss the
                // natural tail of short utterances and caused users to keep
                // holding the recording key. A 650 ms stable window still
                // filters brief revisions while keeping the final check quick.
                now.saturating_duration_since(since) >= Duration::from_millis(650)
            })
    }

    pub fn checking(&mut self) {
        self.phase = Phase::Checking;
        self.phase_since = Instant::now();
    }

    pub fn final_result(&mut self, text: &str) -> bool {
        if self.phase != Phase::Checking {
            self.fail("跟读录音提前结束，请取消后重试".to_owned());
            return false;
        }
        let transcript = self.transcript(text);
        self.current = text.to_owned();
        let Some(matcher) = &self.matcher else {
            return false;
        };
        let progress = matcher.evaluate(&transcript);
        self.matched = progress.matched_words;
        self.stable_since = None;
        if progress.complete {
            self.phase = Phase::Complete;
            true
        } else {
            // 只保留目标句中已匹配的连续前缀。ASR 的 Final 可能把前面
            // 的临时结果修成错词（例如 "chicken" -> "fish"）；若把整段
            // 错误 Final 固定进下一轮，用户从头正确重读时便永远无法对齐。
            // 用规范目标词保存前缀，下一次录音可以从未完成处继续或重读。
            self.confirmed = matcher
                .words()
                .iter()
                .take(progress.matched_words)
                .cloned()
                .collect::<Vec<_>>()
                .join(" ");
            self.current.clear();
            self.feedback.clear();
            self.phase = Phase::Preparing;
            self.phase_since = Instant::now();
            let heard = text.trim();
            self.feedback = if !self.learn_english && heard.is_empty() {
                format!("Didn't catch that; current word: {}", self.current_word())
            } else if !self.learn_english {
                format!(
                    "Heard “{}”; current word: {}",
                    short_text(heard),
                    self.current_word()
                )
            } else if heard.is_empty() {
                format!("没有听清；当前词是 {}", self.current_word())
            } else {
                format!(
                    "识别到“{}”；当前词是 {}",
                    short_text(heard),
                    self.current_word()
                )
            };
            self.current = text.to_owned();
            false
        }
    }

    pub fn retry_current(&mut self) -> bool {
        if self.phase == Phase::Failed
            && self
                .matcher
                .as_ref()
                .is_some_and(|matcher| self.matched == matcher.words().len())
        {
            self.phase = Phase::Complete;
            return true;
        }
        if !self.action_available() {
            return false;
        }
        self.preserve_matched_prefix();
        self.current.clear();
        self.feedback.clear();
        self.stable_since = None;
        self.phase = Phase::Preparing;
        self.phase_since = Instant::now();
        true
    }

    pub fn skip_current(&mut self) -> bool {
        self.advance_current(if self.learn_english {
            "已跳过"
        } else {
            "Skipped"
        })
    }

    pub fn accept_current(&mut self) -> bool {
        self.advance_current(if self.learn_english {
            "已手动确认"
        } else {
            "Accepted"
        })
    }

    fn advance_current(&mut self, result: &str) -> bool {
        if !self.action_available() {
            return false;
        }
        let Some(matcher) = &self.matcher else {
            return false;
        };
        let Some(word) = matcher.words().get(self.matched).cloned() else {
            return false;
        };
        self.matched += 1;
        self.confirmed = matcher
            .words()
            .iter()
            .take(self.matched)
            .cloned()
            .collect::<Vec<_>>()
            .join(" ");
        self.current.clear();
        self.stable_since = None;
        self.feedback = format!("{result}：{word}");
        self.phase = if self.matched == matcher.words().len() {
            Phase::Complete
        } else {
            Phase::Preparing
        };
        self.phase_since = Instant::now();
        true
    }

    fn action_available(&self) -> bool {
        matches!(
            self.phase,
            Phase::Preparing | Phase::Reading | Phase::Failed
        ) && self.matcher.as_ref().is_some_and(|matcher| {
            self.matched < matcher.words().len() || self.phase == Phase::Failed
        })
    }

    fn current_word(&self) -> &str {
        self.matcher
            .as_ref()
            .and_then(|matcher| matcher.words().get(self.matched))
            .map_or(
                if self.learn_english {
                    "英文词"
                } else {
                    "Chinese word"
                },
                String::as_str,
            )
    }

    pub fn show_word_card(&mut self, card: PracticeWordCard) {
        self.word_card = Some(card);
    }

    pub fn close_word_card(&mut self) {
        self.word_card = None;
    }

    pub fn set_word_card_hint(&mut self, hint: Option<String>) {
        if let Some(card) = &mut self.word_card {
            card.hint = hint;
        }
    }

    /// 暂停收音避免扬声器示范被算作用户跟读，保留已读前缀和当前文字。
    pub fn pause_for_pronunciation(&mut self) {
        self.preserve_matched_prefix();
        self.stable_since = None;
        self.phase = Phase::Preparing;
        self.phase_since = Instant::now();
    }

    fn preserve_matched_prefix(&mut self) {
        if let Some(matcher) = &self.matcher {
            self.confirmed = matcher
                .words()
                .iter()
                .take(self.matched)
                .cloned()
                .collect::<Vec<_>>()
                .join(" ");
        }
    }

    pub fn take_original(&mut self) -> Option<String> {
        (self.phase == Phase::Complete && !self.original.is_empty())
            .then(|| std::mem::take(&mut self.original))
    }

    pub fn completed_original(&self) -> Option<&str> {
        (self.phase == Phase::Complete && !self.original.is_empty())
            .then_some(self.original.as_str())
    }

    pub fn fail(&mut self, message: String) {
        self.phase = Phase::Failed;
        self.error = message;
        self.feedback.clear();
        self.stable_since = None;
    }

    pub fn timed_out(&self, now: Instant) -> bool {
        let limit = match self.phase {
            Phase::Translating => Duration::from_secs(125),
            Phase::Demonstrating => Duration::from_secs(
                self.matcher
                    .as_ref()
                    .map_or(0, |matcher| matcher.words().len() as u64)
                    .saturating_mul(3)
                    .saturating_add(30),
            ),
            Phase::Preparing => Duration::from_secs(65),
            Phase::Checking => Duration::from_secs(15),
            _ => return false,
        };
        now.saturating_duration_since(self.phase_since) >= limit
    }

    pub fn view(&self) -> PracticeView {
        PracticeView {
            learn_english: self.learn_english,
            pronunciations: self.pronunciations.clone(),
            original: self.original.clone(),
            words: self
                .matcher
                .as_ref()
                .map_or_else(Vec::new, |matcher| matcher.words().to_vec()),
            matched_words: self.matched,
            level_label: if self.learn_english {
                self.level.label().to_owned()
            } else {
                match self.level {
                    PracticeLevel::Newcomer => "Beginner Lv.1",
                    PracticeLevel::Intermediate => "Intermediate Lv.2",
                    PracticeLevel::Challenge => "Challenge Lv.3",
                }
                .to_owned()
            },
            level_progress: self.level_progress.clone(),
            recognized: self.current.clone(),
            feedback: if self.phase == Phase::Failed {
                self.error.clone()
            } else {
                self.feedback.clone()
            },
            actions_enabled: self.action_available(),
            recording: self.phase == Phase::Reading,
            word_card: self.word_card.clone(),
            status: match self.phase {
                Phase::Translating if self.learn_english => "正在翻译为英语 · 麦克风已关闭",
                Phase::Translating => "Translating to Chinese · Microphone off",
                Phase::Demonstrating if self.learn_english => "示范朗读 · 先听一遍，结束后自动录音",
                Phase::Demonstrating => "Listen first · Recording starts after the example",
                Phase::Preparing if self.learn_english => "正在准备跟读麦克风",
                Phase::Preparing => "Preparing the microphone",
                Phase::Reading if self.demo_unavailable && self.learn_english => {
                    "示范暂不可用 · 请直接跟读 · 正在录音"
                }
                Phase::Reading if self.demo_unavailable => {
                    "Example voice unavailable · Read aloud · Recording"
                }
                Phase::Reading if self.learn_english => "请朗读英文 · 正在录音",
                Phase::Reading => "Read the Chinese aloud · Recording",
                Phase::Checking if self.learn_english => "正在结束录音并核对最后几个词",
                Phase::Checking => "Finishing recording and checking the last words",
                Phase::Failed => &self.error,
                Phase::Complete if self.learn_english => "跟读完成",
                Phase::Complete => "Reading complete",
            }
            .to_owned(),
        }
    }

    fn transcript(&self, text: &str) -> String {
        if self.confirmed.is_empty() {
            text.to_owned()
        } else {
            format!("{} {text}", self.confirmed)
        }
    }
}

fn short_text(text: &str) -> String {
    let mut chars = text.chars();
    let short: String = chars.by_ref().take(36).collect();
    if chars.next().is_some() {
        format!("{short}…")
    } else {
        short
    }
}
