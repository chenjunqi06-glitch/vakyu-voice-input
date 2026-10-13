//! 托盘菜单开关的即时状态与依赖关系。

use qingjian_core::Engine;
use qingjian_dictionary::Dictionary;

use crate::dispatch::{Router, RouterConfig, StatusEvent};

use super::TrayAction;

#[test]
fn practice_menu_requires_voice_and_demo_tracks_practice() {
    let mut router = Router::new(Engine::new(Dictionary::default()), RouterConfig::default());
    router.config.voice_input = false;

    router.handle_status_event(StatusEvent::Tray(TrayAction::ToggleEnglishPractice));
    assert!(!router.config.voice_english_practice);

    router.handle_status_event(StatusEvent::Tray(TrayAction::ToggleVoiceInput));
    router.handle_status_event(StatusEvent::Tray(TrayAction::ToggleEnglishPractice));
    assert!(router.config.voice_english_practice);

    router.handle_status_event(StatusEvent::Tray(TrayAction::TogglePracticeDemo));
    assert!(!router.config.voice_practice_demo);
    assert!(router.tray_state().english_practice);
}

#[test]
fn tray_cloud_switch_changes_the_runtime_state() {
    let mut router = Router::new(Engine::new(Dictionary::default()), RouterConfig::default());
    assert!(!router.tray_state().cloud);

    router.handle_status_event(StatusEvent::Tray(TrayAction::ToggleCloud));

    assert!(router.tray_state().cloud);
}

#[test]
fn learning_direction_switch_is_independent_of_practice_enablement() {
    let mut router = Router::new(Engine::new(Dictionary::default()), RouterConfig::default());
    assert!(router.tray_state().learn_english);
    router.handle_status_event(StatusEvent::Tray(TrayAction::ToggleLearningDirection));
    assert!(!router.tray_state().learn_english);
    assert!(!router.tray_state().english_practice);
    router.handle_status_event(StatusEvent::Tray(TrayAction::ToggleEnglishPractice));
    assert!(!router.tray_state().learn_english);
    assert!(router.tray_state().english_practice);
}

#[test]
fn status_bar_practice_cells_are_mutually_exclusive_toggles() {
    let mut router = Router::new(Engine::new(Dictionary::default()), RouterConfig::default());
    router.config.voice_input = false;

    router.handle_status_event(StatusEvent::Tray(TrayAction::ToggleSendTranslation));
    assert!(
        router.config.voice_input,
        "从状态条开启跟读会顺带打开语音输入"
    );
    assert!(router.tray_state().english_practice);
    assert!(router.tray_state().send_translation);

    router.handle_status_event(StatusEvent::Tray(TrayAction::ToggleShadowing));
    assert!(router.tray_state().english_practice);
    assert!(!router.tray_state().send_translation);

    router.handle_status_event(StatusEvent::Tray(TrayAction::ToggleShadowing));
    assert!(!router.tray_state().english_practice);
    assert!(!router.tray_state().send_translation);
}
