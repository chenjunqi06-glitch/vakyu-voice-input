//! 英文折行与长篇跟读滚动窗口的边界验证。

use super::layout::{visible_range, wrap_words};
use super::{action_at, word_at, word_card_bounds};
use crate::dispatch::{PracticeAction, PracticeView, PracticeWordCard};
use windows::Win32::Foundation::RECT;

fn measure(text: &str) -> i32 {
    text.chars().count() as i32
}

#[test]
fn current_word_stays_visible_at_start_middle_and_end_of_long_text() {
    let words: Vec<_> = (0..150).map(|index| format!("word{index}")).collect();
    let lines = wrap_words(&words, 25, measure);
    assert!(lines.len() > 20);
    for matched in [0, 1, 50, 100, 149] {
        let range = visible_range(&lines, matched);
        assert!(range.len() <= 5);
        assert!(
            lines[range]
                .iter()
                .flatten()
                .any(|word| word.index == matched)
        );
    }
    assert_eq!(visible_range(&lines, 0).start, 0);
    assert_eq!(visible_range(&lines, words.len()).end, lines.len());
    assert!(visible_range(&lines, 100).start > 0);
}

#[test]
fn oversized_word_wraps_without_changing_its_progress_index() {
    let words = vec!["abcdefghij".to_owned(), "next".to_owned()];
    let lines = wrap_words(&words, 4, measure);
    assert!(lines.iter().flatten().all(|run| run.width <= 4));
    let rebuilt: String = lines
        .iter()
        .flatten()
        .filter(|run| run.index == 0)
        .map(|run| run.text.as_str())
        .collect();
    assert_eq!(rebuilt, words[0]);
    assert!(
        lines
            .iter()
            .flatten()
            .any(|run| run.index == 1 && run.text == "next")
    );
}

#[test]
fn translation_pending_has_an_empty_safe_window() {
    let lines = wrap_words(&[], 300, measure);
    assert_eq!(visible_range(&lines, 0), 0..0);
}

#[test]
fn practice_buttons_hit_only_their_drawn_regions() {
    let view = PracticeView {
        learn_english: true,
        pronunciations: Vec::new(),
        original: "我叫陈俊奇".to_owned(),
        words: vec!["Chen".to_owned(), "Junqi.".to_owned()],
        matched_words: 1,
        level_label: "新手 Lv.1".to_owned(),
        level_progress: "0 / 120 XP".to_owned(),
        recognized: "Chen Jing Xi".to_owned(),
        feedback: "识别到“Chen Jing Xi”；当前词是 Junqi.".to_owned(),
        actions_enabled: true,
        recording: false,
        word_card: None,
        status: "本轮未通过".to_owned(),
    };
    let size = (540, 600);
    assert_eq!(
        action_at(&view, 30, 570, size, 96),
        Some(PracticeAction::Retry)
    );
    assert_eq!(
        action_at(&view, 200, 570, size, 96),
        Some(PracticeAction::SkipCurrent)
    );
    assert_eq!(
        action_at(&view, 380, 570, size, 96),
        Some(PracticeAction::AcceptCurrent)
    );
    assert_eq!(action_at(&view, 185, 570, size, 96), None);
    assert_eq!(action_at(&view, 30, 500, size, 96), None);
}

#[test]
fn clicking_a_visible_word_opens_its_card_without_retry_state() {
    let view = sample_view();
    assert_eq!(
        word_at(
            &view,
            42,
            130,
            &[(
                RECT {
                    left: 30,
                    top: 120,
                    right: 80,
                    bottom: 150,
                },
                2,
            )],
        ),
        Some(PracticeAction::LookupWord(2))
    );
}

#[test]
fn word_card_actions_remain_available_during_recording() {
    let mut view = sample_view();
    view.recording = true;
    view.word_card = Some(PracticeWordCard {
        word: "hello".to_owned(),
        meaning: Some("你好".to_owned()),
        pronunciation: Some("/həˈloʊ/".to_owned()),
        hint: None,
    });
    let (_, play, close) = word_card_bounds(540, 96);
    assert_eq!(
        action_at(&view, play.left + 1, play.top + 1, (540, 600), 96),
        Some(PracticeAction::SpeakWord)
    );
    assert_eq!(
        action_at(&view, close.left + 1, close.top + 1, (540, 600), 96),
        Some(PracticeAction::CloseWordCard)
    );
    assert_eq!(word_at(&view, 42, 130, &[(RECT::default(), 0)]), None);
}

fn sample_view() -> PracticeView {
    PracticeView {
        learn_english: true,
        pronunciations: Vec::new(),
        original: "你好".to_owned(),
        words: vec!["hello".to_owned()],
        matched_words: 0,
        level_label: "新手 Lv.1".to_owned(),
        level_progress: "0 / 5".to_owned(),
        recognized: String::new(),
        feedback: String::new(),
        actions_enabled: false,
        recording: false,
        word_card: None,
        status: "正在录音".to_owned(),
    }
}

/// 只合成本程序的固定帧，不采集麦克风、不读取其他应用或屏幕。
#[test]
#[ignore = "显式导出固定词卡 PNG，需提供绝对输出路径"]
fn export_offline_word_card_preview() {
    let output = std::env::var_os("MIRACLE_WORD_CARD_PREVIEW").expect("预览 PNG 路径");
    let window = crate::ui::voice::VoiceWindow::new(std::sync::Arc::new(|_| {}))
        .expect("创建无焦点预览窗口");
    let mut view = sample_view();
    view.original = "我的名字是陈俊奇，我今天要去吃饭。".to_owned();
    view.words = "My name is Chen Junqi and I want to eat today."
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    view.matched_words = 3;
    view.recognized = "My name is".to_owned();
    view.actions_enabled = true;
    view.recording = true;
    view.word_card = Some(PracticeWordCard {
        word: "today".to_owned(),
        meaning: Some("adv. 今天；现今".to_owned()),
        pronunciation: Some("/təˈdeɪ/".to_owned()),
        hint: None,
    });
    crate::ui::layered::export_next_frame(output.into()).unwrap();
    window
        .render_practice(&view, RECT::default())
        .expect("导出词卡帧");
}

/// 中文行内拼音与词卡共用生产绘制路径；不启动录音和网络。
#[test]
#[ignore = "显式导出双向跟读 PNG，需提供绝对输出目录"]
fn export_chinese_practice_preview() {
    let output =
        std::path::PathBuf::from(std::env::var_os("VAKYU_CHINESE_PREVIEW").expect("预览目录"));
    std::fs::create_dir_all(&output).unwrap();
    let window = crate::ui::voice::VoiceWindow::new(std::sync::Arc::new(|_| {})).unwrap();
    let mut view = sample_view();
    view.learn_english = false;
    view.original = "I want to go to the bank today and eat with my friends tomorrow.".to_owned();
    let matcher = qingjian_learning::ChinesePractice::with_level_and_config(
        "我今天想去银行，明天和朋友一起吃饭。",
        qingjian_learning::PracticeLevel::Newcomer,
        qingjian_learning::PracticeMatchConfig::default(),
    );
    view.words = matcher.words().to_vec();
    view.pronunciations = qingjian_translate::chinese_practice_readings(&view.words);
    view.matched_words = 4;
    view.recognized = "我今天想".to_owned();
    view.level_label = "Beginner Lv.1".to_owned();
    view.level_progress = "0 / 120 XP".to_owned();
    view.status = "Read the Chinese aloud · Recording".to_owned();
    view.actions_enabled = true;
    view.recording = true;
    crate::ui::layered::export_next_frame(output.join("chinese-practice.png")).unwrap();
    window.render_practice(&view, RECT::default()).unwrap();
    view.word_card = Some(PracticeWordCard {
        word: "银行".to_owned(),
        meaning: qingjian_translate::offline_chinese_meaning("银行"),
        pronunciation: qingjian_translate::chinese_pinyin("银行"),
        hint: None,
    });
    crate::ui::layered::export_next_frame(output.join("chinese-word-card.png")).unwrap();
    window.render_practice(&view, RECT::default()).unwrap();
}
