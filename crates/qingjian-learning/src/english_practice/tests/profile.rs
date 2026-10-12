//! 经验累积、单轮去重、分级边界和损坏文件保留。

use std::{fs, io, path::PathBuf};

use crate::english_practice::{PracticeLevel, PracticeProfile, PracticeProgress};

fn complete(words: usize) -> PracticeProgress {
    PracticeProgress {
        matched_words: words,
        total_words: words,
        complete: true,
    }
}

fn scratch(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "qingjian-practice-{}-{name}.json",
        std::process::id()
    ))
}

#[test]
fn new_profile_starts_easy_and_each_round_only_earns_once() {
    let mut profile = PracticeProfile::default();
    assert_eq!(profile.level(), PracticeLevel::Newcomer);
    assert_eq!(profile.xp(), 0);
    assert_eq!(profile.completed_sessions(), 0);
    let mut round = profile.start_round();
    assert_eq!(round.level(), PracticeLevel::Newcomer);
    assert_eq!(
        profile.award_completed(&mut round, PracticeProgress::default()),
        0
    );
    assert_eq!(profile.award_completed(&mut round, complete(0)), 0);
    assert_eq!(profile.award_completed(&mut round, complete(8)), 8);
    assert_eq!(profile.award_completed(&mut round, complete(8)), 0);
    assert_eq!(profile.xp(), 8);
    assert_eq!(profile.completed_sessions(), 1);
}

#[test]
fn long_round_is_capped_and_levels_only_increase_after_completed_rounds() {
    let mut profile = PracticeProfile::default();
    let mut first = profile.start_round();
    assert_eq!(profile.award_completed(&mut first, complete(1081)), 60);
    assert_eq!(profile.level(), PracticeLevel::Newcomer);
    let mut second = profile.start_round();
    assert_eq!(profile.award_completed(&mut second, complete(60)), 60);
    assert_eq!(second.level(), PracticeLevel::Newcomer);
    assert_eq!(profile.level(), PracticeLevel::Intermediate);
    assert_eq!(profile.start_round().level(), PracticeLevel::Intermediate);
    let mut failed = profile.start_round();
    assert_eq!(
        profile.award_completed(
            &mut failed,
            PracticeProgress {
                matched_words: 9,
                total_words: 10,
                complete: false,
            }
        ),
        0
    );
    assert_eq!(profile.xp(), 120);
    assert_eq!(profile.completed_sessions(), 2);
    for _ in 0..6 {
        let mut round = profile.start_round();
        profile.award_completed(&mut round, complete(60));
    }
    assert_eq!(profile.xp(), 480);
    assert_eq!(profile.level(), PracticeLevel::Intermediate);
    let mut final_round = profile.start_round();
    profile.award_completed(&mut final_round, complete(20));
    assert_eq!(profile.level(), PracticeLevel::Challenge);
    assert_eq!(profile.level().next_threshold(), None);
}

#[test]
fn level_labels_and_thresholds_are_stable() {
    assert_eq!(PracticeLevel::from_xp(119).label(), "新手 Lv.1");
    assert_eq!(PracticeLevel::from_xp(120).label(), "进阶 Lv.2");
    assert_eq!(PracticeLevel::from_xp(499).next_threshold(), Some(500));
    assert_eq!(PracticeLevel::from_xp(500).label(), "挑战 Lv.3");
    assert_eq!(PracticeLevel::Newcomer.next_threshold(), Some(120));
}

#[test]
fn profile_round_trip_only_stores_two_statistics() {
    let path = scratch("round-trip");
    let mut profile = PracticeProfile::default();
    let mut round = profile.start_round();
    profile.award_completed(&mut round, complete(12));
    profile.save(&path).unwrap();
    assert_eq!(PracticeProfile::load(&path).unwrap(), profile);
    let json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    let object = json.as_object().unwrap();
    assert_eq!(object.len(), 2);
    assert_eq!(object["xp"], 12);
    assert_eq!(object["completed_sessions"], 1);
    fs::remove_file(path).unwrap();
}

#[test]
fn missing_file_defaults_but_corruption_is_preserved_as_error() {
    let path = scratch("broken");
    let _ = fs::remove_file(&path);
    assert_eq!(
        PracticeProfile::load(&path).unwrap(),
        PracticeProfile::default()
    );
    for original in ["{broken", "{}", r#"{"xp":-1,"completed_sessions":1}"#] {
        fs::write(&path, original).unwrap();
        let error = PracticeProfile::load(&path).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
    }
    fs::remove_file(path).unwrap();
}

#[test]
fn counters_saturate_without_panicking_or_downgrading() {
    let mut profile: PracticeProfile = serde_json::from_str(&format!(
        r#"{{"xp":{},"completed_sessions":{}}}"#,
        u64::MAX - 2,
        u64::MAX
    ))
    .unwrap();
    let mut round = profile.start_round();
    assert_eq!(profile.award_completed(&mut round, complete(10)), 2);
    assert_eq!(profile.xp(), u64::MAX);
    assert_eq!(profile.completed_sessions(), u64::MAX);
    assert_eq!(profile.level(), PracticeLevel::Challenge);
}
