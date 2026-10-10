//! 模拟服务端的截断与完整回复，确保完整性检查在 JSON 解析之前执行。

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;

use crate::chat_client::ChatClient;
use crate::config::PredictConfig;
use crate::error::PredictError;

#[test]
fn even_valid_json_is_rejected_when_server_marks_it_truncated() {
    assert!(matches!(
        mock_response("length"),
        Err(PredictError::IncompleteReply)
    ));
    assert_eq!(
        mock_response("stop").unwrap(),
        r#"{"english":"I am going out to eat today."}"#
    );
}

fn mock_response(finish_reason: &str) -> Result<String, PredictError> {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base_url = format!("http://{}/v1", listener.local_addr().unwrap());
    let response = serde_json::json!({
        "id": "voice-practice-test", "object": "chat.completion", "created": 0,
        "model": "test-model", "choices": [{
            "index": 0, "finish_reason": finish_reason,
            "message": { "role": "assistant", "content": "{\"english\":\"I am going out to eat today.\"}" }
        }]
    }).to_string();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(&stream);
        let mut length = 0;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if line == "\r\n" || line.is_empty() {
                break;
            }
            if let Some((name, value)) = line.split_once(':')
                && name.eq_ignore_ascii_case("content-length")
            {
                length = value.trim().parse::<usize>().unwrap();
            }
        }
        reader.read_exact(&mut vec![0_u8; length]).unwrap();
        let headers = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
            response.len()
        );
        stream.write_all(headers.as_bytes()).unwrap();
        stream.write_all(response.as_bytes()).unwrap();
    });
    let client = ChatClient::new(
        &PredictConfig {
            base_url,
            timeout_ms: 5000,
            ..PredictConfig::default()
        },
        "test-only-placeholder".to_owned(),
    );
    let result = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(client.chat_private("Translate.", "我今天要去吃饭。", 8192));
    server.join().unwrap();
    result
}
