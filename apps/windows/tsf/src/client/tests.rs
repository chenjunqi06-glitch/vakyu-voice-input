//! 内存双工流验证语音桥的序号与每次控制回话，不启动 Server 或麦克风。

use qingjian_platform::protocol::{
    ClientMessage, Frame, InputSettings, ServerMessage, SessionId, VoiceAction, VoiceUpdate,
    read_message, write_message,
};
use std::cell::RefCell;
use std::io::{self, Cursor, Read, Write};
use std::rc::Rc;

use super::EngineClient;

#[test]
fn external_delivery_roundtrip_preserves_host_identity_and_ack() {
    use qingjian_platform::protocol::{IndicatorState, VoiceDeliveryCommand, VoiceDeliveryReport};
    let session = SessionId(711);
    let mut input = Vec::new();
    write_message(
        &mut input,
        &ServerMessage::SessionOpened {
            session,
            input: InputSettings {
                voice_delivery: true,
                ..Default::default()
            },
        },
    )
    .unwrap();
    write_message(
        &mut input,
        &ServerMessage::VoiceDelivery {
            session,
            input: InputSettings::default(),
            english: false,
            indicator: IndicatorState::default(),
            command: Some(VoiceDeliveryCommand::Commit {
                request: 42,
                text: "你好".to_owned(),
                deadline_ms: 999999,
            }),
        },
    )
    .unwrap();
    let output = Rc::new(RefCell::new(Vec::new()));
    let (mut client, settings) = EngineClient::open(
        Duplex {
            input: Cursor::new(input),
            output: output.clone(),
        },
        session,
        None,
    )
    .unwrap();
    assert!(settings.voice_delivery);
    assert!(!settings.voice_input, "回投能力不授予DLL录音权");
    assert!(matches!(
        client
            .voice_delivery_poll(100, 200, Some(VoiceDeliveryReport::Ready { request: 42 }))
            .unwrap(),
        ServerMessage::VoiceDelivery {
            command: Some(VoiceDeliveryCommand::Commit { request: 42, .. }),
            ..
        }
    ));
    let mut written = Cursor::new(output.borrow().clone());
    read_message::<_, ClientMessage>(&mut written).unwrap();
    assert_eq!(
        read_message::<_, ClientMessage>(&mut written).unwrap(),
        Some(ClientMessage::VoiceDeliveryPoll {
            session,
            process: 100,
            thread: 200,
            report: Some(VoiceDeliveryReport::Ready { request: 42 })
        })
    );
}

struct Duplex {
    input: Cursor<Vec<u8>>,

    output: Rc<RefCell<Vec<u8>>>,
}

impl Read for Duplex {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.input.read(bytes)
    }
}

impl Write for Duplex {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.output.borrow_mut().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn every_voice_control_consumes_its_reply_and_preserves_the_sequence() {
    let session = SessionId(711);
    let mut input = Vec::new();
    write_message(
        &mut input,
        &ServerMessage::SessionOpened {
            session,
            input: InputSettings::default(),
        },
    )
    .unwrap();
    let updates = [
        Some(VoiceUpdate::Armed),
        Some(VoiceUpdate::Starting),
        Some(VoiceUpdate::Final {
            text: String::new(),
        }),
        None,
        None,
    ];
    for voice in updates.clone() {
        write_message(
            &mut input,
            &ServerMessage::Update {
                session,
                frame: Frame::default(),
                voice,
            },
        )
        .unwrap();
    }
    let output = Rc::new(RefCell::new(Vec::new()));
    let (mut client, _) = EngineClient::open(
        Duplex {
            input: Cursor::new(input),
            output: output.clone(),
        },
        session,
        None,
    )
    .unwrap();
    let actions = [
        VoiceAction::Press,
        VoiceAction::Poll,
        VoiceAction::Release,
        VoiceAction::Cancel,
    ];
    for (action, expected) in actions.into_iter().zip(updates) {
        assert_eq!(client.voice_control(42, action).unwrap().voice, expected);
    }
    assert!(
        client.poll_update().unwrap().voice.is_none(),
        "Cancel回话不能残留给下一次请求"
    );
    let mut written = Cursor::new(output.borrow().clone());
    assert!(matches!(
        read_message::<_, ClientMessage>(&mut written).unwrap(),
        Some(ClientMessage::OpenSession { .. })
    ));
    for action in actions {
        assert_eq!(
            read_message::<_, ClientMessage>(&mut written).unwrap(),
            Some(ClientMessage::VoiceControl {
                session,
                sequence: 42,
                action
            })
        );
    }
    assert!(matches!(
        read_message::<_, ClientMessage>(&mut written).unwrap(),
        Some(ClientMessage::Poll { .. })
    ));
}
