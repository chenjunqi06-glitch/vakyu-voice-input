//! Server 返回的语音生命周期与全文结果；Armed 仅用于协商后的新桥。

use serde::{Deserialize, Serialize};

/// Server-side status or transcript produced by a voice session.
///
/// Voice recognition is asynchronous: `Started` acknowledges a start request,
/// `Interim` carries the latest partial transcript, and `Final` carries the
/// text that should be committed when recording stops. `Error` is terminal for
/// the current voice request/session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VoiceUpdate {
    /// 新语音桥已登记按住，尚未达到长按阈值；旧消息路径不会收到此变体。
    Armed,

    /// 正在准备识别器，还没有打开麦克风。
    Starting,

    /// Recording and recognition were started.
    Started,

    /// 已停止收音；尾部识别完成后由轮询交出 Final。
    Stopping,

    /// Latest partial transcript while recording is held.
    Interim { text: String },

    /// Final transcript produced when recording stops.
    Final { text: String },

    /// The voice backend could not start or process the session.
    Error { message: String },
}

#[cfg(test)]
mod tests {
    use super::VoiceUpdate;

    #[test]
    fn old_server_cannot_enable_voice_messages_it_does_not_understand() {
        let mut input = serde_json::to_value(crate::protocol::InputSettings {
            voice_input: true,
            voice_bridge_version: 1,
            ..Default::default()
        })
        .unwrap();
        input.as_object_mut().unwrap().remove("voice_input");
        let decoded: crate::protocol::InputSettings = serde_json::from_value(input).unwrap();
        assert!(!decoded.voice_input);
    }

    #[test]
    fn missing_bridge_capability_keeps_new_dll_on_the_old_protocol() {
        let mut input = serde_json::to_value(crate::protocol::InputSettings {
            voice_input: true,
            voice_bridge_version: 1,
            ..Default::default()
        })
        .unwrap();
        input
            .as_object_mut()
            .unwrap()
            .remove("voice_bridge_version");
        input.as_object_mut().unwrap().remove("voice_key");
        let decoded: crate::protocol::InputSettings = serde_json::from_value(input).unwrap();
        assert_eq!(decoded.voice_bridge_version, 0);
        assert_eq!(decoded.voice_key, crate::VoiceKey::Alt);
    }

    #[test]
    fn old_mode_sync_without_input_cannot_advertise_voice_or_the_new_bridge() {
        let decoded: crate::protocol::ServerMessage =
            serde_json::from_str(r#"{"ModeSync":{"session":42,"english":null}}"#).unwrap();
        let crate::protocol::ServerMessage::ModeSync { input, .. } = decoded else {
            panic!("应解析旧 ModeSync 回包");
        };
        assert!(!input.voice_input);
        assert_eq!(input.voice_bridge_version, 0);
        assert_eq!(input.voice_key, crate::VoiceKey::Alt);
        assert!(!input.voice_delivery);
    }

    #[test]
    fn bridge_actions_preserve_session_and_press_sequence() {
        use crate::protocol::{ClientMessage, SessionId, VoiceAction};
        for action in [
            VoiceAction::Press,
            VoiceAction::Poll,
            VoiceAction::Release,
            VoiceAction::Cancel,
        ] {
            let message = ClientMessage::VoiceControl {
                session: SessionId(42),
                sequence: 8,
                action,
            };
            let decoded =
                serde_json::from_value::<ClientMessage>(serde_json::to_value(&message).unwrap())
                    .unwrap();
            assert_eq!(decoded, message);
        }
        assert_eq!(
            serde_json::to_value(ClientMessage::PrepareUpdate).unwrap(),
            "PrepareUpdate"
        );
    }

    #[test]
    fn voice_updates_round_trip_as_json() {
        for update in [
            VoiceUpdate::Armed,
            VoiceUpdate::Starting,
            VoiceUpdate::Started,
            VoiceUpdate::Stopping,
            VoiceUpdate::Interim {
                text: "你好".to_owned(),
            },
            VoiceUpdate::Final {
                text: "你好，世界".to_owned(),
            },
            VoiceUpdate::Error {
                message: "microphone unavailable".to_owned(),
            },
        ] {
            let encoded = serde_json::to_vec(&update).expect("serialize voice update");
            let decoded: VoiceUpdate =
                serde_json::from_slice(&encoded).expect("deserialize voice update");
            assert_eq!(decoded, update);
        }
    }
}
