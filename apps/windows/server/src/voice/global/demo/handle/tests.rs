//! 通道回归与显式启用的短示范验证；普通测试不访问扬声器，所有测试都不开麦。

use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use super::EnglishDemo;
use crate::voice::global::demo::command::Command;
use crate::voice::global::demo::result::DemoResult;
use qingjian_core::Language;

fn channels() -> (
    EnglishDemo,
    mpsc::Receiver<Command>,
    mpsc::Sender<(u64, DemoResult)>,
) {
    let (commands, command_rx) = mpsc::channel();
    let (result_tx, results) = mpsc::channel();
    (
        EnglishDemo {
            commands,
            results,
            desired: Arc::new(AtomicU64::new(0)),
            generation: 0,
            active: None,
            natural: None,
        },
        command_rx,
        result_tx,
    )
}

#[test]
fn demo_selects_each_language_without_reusing_previous_voice() {
    let (mut demo, commands, _results) = channels();
    for (language, text) in [(Language::Chinese, "你好"), (Language::English, "Hello")] {
        demo.speak_in_language(11, text, -2, language).unwrap();
        assert!(matches!(commands.recv().unwrap(), Command::Cancel));
        assert!(
            matches!(commands.recv().unwrap(), Command::Speak { text: actual, language: selected, rate: -2, .. } if actual == text && selected == language)
        );
    }
    demo.speak(12, "Welcome", 0).unwrap();
    assert!(matches!(commands.recv().unwrap(), Command::Cancel));
    assert!(matches!(
        commands.recv().unwrap(),
        Command::Speak {
            language: Language::English,
            ..
        }
    ));
}

#[test]
fn cancelled_and_replaced_playback_cannot_complete_current_round() {
    let (mut demo, _commands, results) = channels();
    demo.speak(11, "Hello.", -2).unwrap();
    let old = demo.generation;
    demo.cancel();
    results
        .send((
            old,
            DemoResult {
                request_id: 11,
                result: Ok(()),
            },
        ))
        .unwrap();
    assert!(demo.try_recv().is_none());
    demo.speak(12, "Good morning.", -1).unwrap();
    results
        .send((
            old,
            DemoResult {
                request_id: 11,
                result: Ok(()),
            },
        ))
        .unwrap();
    assert!(demo.try_recv().is_none());
    results
        .send((
            demo.generation,
            DemoResult {
                request_id: 12,
                result: Ok(()),
            },
        ))
        .unwrap();
    let completed = demo.try_recv().unwrap();
    assert_eq!(completed.request_id, 12);
    assert!(completed.result.is_ok());
    assert!(demo.try_recv().is_none());
}

#[test]
fn disconnected_player_reports_failure_once_instead_of_starting_followup() {
    let (mut demo, _commands, results) = channels();
    demo.speak(31, "I can learn English.", -2).unwrap();
    drop(results);
    let completed = demo.try_recv().unwrap();
    assert_eq!(completed.request_id, 31);
    assert!(completed.result.is_err());
    assert!(demo.try_recv().is_none());
}

#[test]
#[ignore = "会从默认扬声器播放一句固定英文；验证真实 SAPI 完成事件，不打开麦克风"]
fn short_english_demo_reports_real_playback_completion() {
    let mut demo = EnglishDemo::new().unwrap();
    let request_id = 901;
    demo.speak(request_id, "Hello. Let us practice English.", -2)
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut completion = None;
    while Instant::now() < deadline {
        if let Some(result) = demo.try_recv() {
            completion = Some(result);
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    // 超时或失败同样停止播放；不能让测试结束后还留着示范音频。
    demo.cancel();
    let completed = completion.expect("英语示范在十秒内未收到音频结束事件");
    assert_eq!(completed.request_id, request_id);
    assert!(completed.result.is_ok(), "{:?}", completed.result);
    assert!(demo.try_recv().is_none());
}

#[test]
fn natural_voice_setting_travels_with_each_request() {
    let (mut demo, commands, _results) = channels();
    demo.set_natural_voice(Some(20));
    demo.speak(41, "Hello.", -2).unwrap();
    assert!(matches!(commands.recv().unwrap(), Command::Cancel));
    assert!(matches!(
        commands.recv().unwrap(),
        Command::Speak {
            natural: Some(20),
            ..
        }
    ));
    demo.set_natural_voice(None);
    demo.speak(42, "Hello.", -2).unwrap();
    assert!(matches!(commands.recv().unwrap(), Command::Cancel));
    assert!(matches!(
        commands.recv().unwrap(),
        Command::Speak { natural: None, .. }
    ));
}

#[test]
#[ignore = "需要完整安装包的自然人声组件；会从默认扬声器播放一句固定英文，不打开麦克风"]
fn short_english_demo_plays_with_natural_voice() {
    let mut demo = EnglishDemo::new().unwrap();
    demo.set_natural_voice(Some(20));
    let request_id = 902;
    demo.speak(request_id, "Hello. Let us practice English.", -2)
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut completion = None;
    while Instant::now() < deadline {
        if let Some(result) = demo.try_recv() {
            completion = Some(result);
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    demo.cancel();
    let completed = completion.expect("自然人声示范在一分钟内未结束");
    assert_eq!(completed.request_id, request_id);
    assert!(completed.result.is_ok(), "{:?}", completed.result);
}
