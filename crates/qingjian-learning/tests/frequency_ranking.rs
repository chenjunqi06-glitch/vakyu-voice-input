//! 用真实学习器验证候选选择、跨拼写排序与独立临时目录中的持久化。

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use qingjian_core::{CandidateKind, Engine, Learner};
use qingjian_dictionary::Dictionary;
use qingjian_learning::FrequencyLearner;

const DICTIONARY: &str = "拿\tna\t10000\n哪\tna\t200\n那\tna\t100\n那\tnai\t2\n奶\tnai\t10000\n开发\tkai fa\t10000\n开阀\tkai fa\t100\n";

fn engine(learner: FrequencyLearner) -> Engine {
    Engine::new(Dictionary::parse(DICTIONARY).unwrap()).with_learner(Box::new(learner))
}

fn select(engine: &mut Engine, input: &str, text: &str) {
    engine.set_input(input);
    let candidate = engine
        .query()
        .unwrap()
        .candidates
        .items
        .into_iter()
        .find(|candidate| candidate.kind == CandidateKind::Chinese && candidate.text == text)
        .unwrap_or_else(|| panic!("{input} has no candidate {text}"));
    assert_eq!(engine.commit(&candidate), text);
}

fn first(engine: &mut Engine, input: &str) -> String {
    engine.set_input(input);
    engine.query().unwrap().candidates.items[0].text.clone()
}

fn temporary_directory() -> PathBuf {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "vakyu-frequency-ranking-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&path).unwrap();
    path
}

#[test]
fn repeated_choices_reorder_and_survive_a_restart() {
    let directory = temporary_directory();
    let path = directory.join("user.tsv");
    let mut live = engine(FrequencyLearner::from_path(&path).unwrap());
    assert_eq!(first(&mut live, "na"), "拿");
    select(&mut live, "na", "拿");
    for _ in 0..3 {
        live.note_passthrough('\n');
        select(&mut live, "na", "那");
    }
    assert_eq!(first(&mut live, "na"), "那");
    assert_eq!(live.learner().choice_weight("na", "那"), 3);
    live.flush_learning();

    let mut restored = engine(FrequencyLearner::from_path(&path).unwrap());
    assert_eq!(first(&mut restored, "na"), "那");
    assert_eq!(restored.learner().weight("那"), 3);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn abbreviated_and_prefix_choices_teach_the_complete_pinyin() {
    let directory = temporary_directory();
    let path = directory.join("user.tsv");
    let mut live = engine(FrequencyLearner::from_path(&path).unwrap());
    for (full, prefix, preferred, old) in
        [("na", "n", "那", "拿"), ("kaifa", "kaif", "开阀", "开发")]
    {
        select(&mut live, full, old);
        for _ in 0..3 {
            live.note_passthrough('\n');
            select(&mut live, prefix, preferred);
        }
        assert_eq!(first(&mut live, full), preferred);
        assert_eq!(live.learner().choice_weight(full, preferred), 3);
        assert_eq!(live.learner().choice_weight(prefix, preferred), 3);
        assert_eq!(live.learner().weight(preferred), 3);
    }
    assert_eq!(live.learner().choice_weight("nai", "那"), 0);
    live.flush_learning();
    let mut restored = engine(FrequencyLearner::from_path(&path).unwrap());
    assert_eq!(first(&mut restored, "na"), "那");
    assert_eq!(first(&mut restored, "kai'fa"), "开阀");
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn the_most_used_choice_wins_without_changing_static_dictionary_frequency() {
    let mut learner = FrequencyLearner::parse("那\t7\n哪\t1\n拿\t2\n");
    for (text, count) in [("那", 7), ("哪", 1), ("拿", 2)] {
        for _ in 0..count {
            learner.record_choice("na", text);
        }
    }
    let mut live = engine(learner);
    assert_eq!(first(&mut live, "na"), "那");
    select(&mut live, "na", "那");
    assert_eq!(live.learner().weight("那"), 8);
    assert_eq!(live.learner().choice_weight("na", "那"), 8);
}

#[test]
fn correcting_an_abbreviated_choice_retracts_both_spellings() {
    let mut live = engine(FrequencyLearner::default());
    select(&mut live, "n", "那");
    live.note_backspace();
    select(&mut live, "na", "拿");
    assert_eq!(live.learner().weight("那"), 0);
    assert_eq!(live.learner().choice_weight("n", "那"), 0);
    assert_eq!(live.learner().choice_weight("na", "那"), 0);
    assert_eq!(live.learner().choice_weight("na", "拿"), 1);
    assert_eq!(first(&mut live, "na"), "拿");
}

#[test]
fn a_different_complete_syllable_does_not_retract_an_abbreviated_choice() {
    let dictionary = Dictionary::parse("南\tnan\t10000\n那\tna\t100\n").unwrap();
    let mut live = Engine::new(dictionary).with_learner(Box::new(FrequencyLearner::default()));
    select(&mut live, "n", "南");
    live.note_backspace();
    select(&mut live, "na", "那");
    assert_eq!(live.learner().weight("南"), 1);
    assert_eq!(live.learner().choice_weight("n", "南"), 1);
    assert_eq!(live.learner().choice_weight("nan", "南"), 1);
    assert_eq!(live.learner().choice_weight("na", "那"), 1);
}

#[test]
fn private_and_disabled_learning_never_record_either_spelling() {
    for (private, learning) in [(false, false), (true, true)] {
        let mut live = engine(FrequencyLearner::default());
        live.set_private(private);
        live.set_learning(learning);
        select(&mut live, "n", "那");
        assert_eq!(live.learner().weight("那"), 0);
        assert_eq!(live.learner().choice_weight("n", "那"), 0);
        assert_eq!(live.learner().choice_weight("na", "那"), 0);
    }
}
