//! 粘贴兼容开关仅改变原生目标策略，配置热更新不打断当前录音。
use super::{controller, validated_value};
use crate::protocol::{Command, Effect, Event, Mode};
use serde_json::json;

#[test]
fn paste_fallback_is_boolean_and_config_event_preserves_false() {
    assert!(validated_value("voice_paste_fallback", &json!(false)).is_ok());
    assert!(validated_value("voice_paste_fallback", &json!("false")).is_err());
    let mut c = controller();
    c.command(Command::Press {
        mode: Some(Mode::Dictation),
    });
    c.drain();
    let mut config = c.config.clone();
    config.shortcut.voice_paste_fallback = false;
    c.apply_config(config);
    let effects = c.drain();
    assert!(effects.iter().any(|effect| matches!(
        effect,
        Effect::Native(Event::Config {
            paste_fallback: false,
            ..
        })
    )));
    assert!(c.active);
    assert!(!c.requires_new_round);
}
