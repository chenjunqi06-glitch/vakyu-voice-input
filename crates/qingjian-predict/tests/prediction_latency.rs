//! 本地 HTTP 回归：合并在途请求，缓存命中不等待防抖。

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use qingjian_core::{Prediction, PredictionKind, PredictionRequest, Predictor};
use qingjian_predict::{CloudPredictor, PredictConfig};

const FIRST: &str = "我先去吃饭";
const MIDDLE: &str = "我现在去上班";
const LATEST: &str = "我要喝水";
const WAIT: Duration = Duration::from_secs(8);
const ENGLISH_MARKER: &str = "\u{1f}QJ_ENGLISH\u{1f}";

#[test]
fn typing_during_a_slow_request_coalesces_to_the_latest_state() {
    let server = MockServer::new(true);
    let mut predictor = server.predictor(50);
    predictor.submit(request(1, FIRST));
    assert!(server.received.recv_timeout(WAIT).unwrap().contains(FIRST));

    predictor.submit(request(2, MIDDLE));
    predictor.submit(request(3, LATEST));
    thread::sleep(Duration::from_millis(100));
    assert!(matches!(
        server.received.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));

    server.release_slow.send(()).unwrap();
    assert!(server.received.recv_timeout(WAIT).unwrap().contains(LATEST));
    assert_prediction(wait_prediction(&mut predictor, WAIT), 3, "I want water.");

    predictor.submit(request(4, FIRST));
    assert_prediction(
        wait_prediction(&mut predictor, Duration::from_secs(1)),
        4,
        "I will eat first.",
    );
    // 中间输入没有发请求，首个请求的结果也能从缓存立即返回。
    assert!(matches!(
        server.received.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
}

#[test]
fn cached_latest_request_bypasses_long_debounce() {
    let server = MockServer::new(false);
    let mut predictor = server.predictor(3_000);
    predictor.submit(request(1, LATEST));
    assert!(server.received.recv_timeout(WAIT).unwrap().contains(LATEST));
    assert_prediction(wait_prediction(&mut predictor, WAIT), 1, "I want water.");

    predictor.submit(request(2, LATEST));
    // 一秒是宽松的失败上限，仍短于三秒防抖，不以毫秒级跑分判断成败。
    assert_prediction(
        wait_prediction(&mut predictor, Duration::from_secs(1)),
        2,
        "I want water.",
    );
    assert!(matches!(
        server.received.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
}

#[test]
fn cached_request_returns_while_newer_http_request_is_busy() {
    let server = MockServer::new(false);
    let mut predictor = server.predictor(3_000);
    predictor.submit(request(1, FIRST));
    assert!(server.received.recv_timeout(WAIT).unwrap().contains(FIRST));
    assert_prediction(
        wait_prediction(&mut predictor, WAIT),
        1,
        "I will eat first.",
    );

    server.block_next.store(true, Ordering::Release);
    predictor.submit(request(2, LATEST));
    assert!(server.received.recv_timeout(WAIT).unwrap().contains(LATEST));
    // LATEST 的 HTTP 响应被闸门挡住；重提已缓存的 FIRST 不应再等三秒防抖。
    predictor.submit(request(3, FIRST));
    assert_prediction(
        wait_prediction(&mut predictor, Duration::from_secs(1)),
        3,
        "I will eat first.",
    );
    assert!(matches!(
        server.received.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
}

fn request(sequence: u64, text: &str) -> PredictionRequest {
    let pinyin = match text {
        FIRST => "wo'xian'qu'chi'fan",
        MIDDLE => "wo'xian'zai'qu'shang'ban",
        _ => "wo'yao'he'shui",
    };
    PredictionRequest {
        sequence,
        kind: PredictionKind::Compose,
        before: String::new(),
        after: String::new(),
        pinyin: pinyin.into(),
        letters: pinyin.replace('\'', ""),
        syllables: pinyin.split('\'').count(),
        candidates: vec![text.into()],
        guess: text.into(),
        max_items: 0,
        want_sentence: false,
        text: String::new(),
        target_language: String::new(),
    }
}

fn wait_prediction(predictor: &mut CloudPredictor, timeout: Duration) -> Prediction {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(prediction) = predictor.poll() {
            return prediction;
        }
        assert!(Instant::now() < deadline, "等待联想结果超时");
        thread::sleep(Duration::from_millis(5));
    }
}

fn assert_prediction(prediction: Prediction, sequence: u64, english: &str) {
    assert_eq!(prediction.sequence, sequence);
    assert!(prediction.words.is_empty());
    assert_eq!(
        prediction.sentence,
        Some(format!("{ENGLISH_MARKER}{english}"))
    );
}

struct MockServer {
    address: SocketAddr,

    received: mpsc::Receiver<String>,

    release_slow: mpsc::Sender<()>,

    block_next: Arc<AtomicBool>,

    stopped: Arc<AtomicBool>,

    thread: Option<JoinHandle<()>>,
}

impl MockServer {
    fn new(block_first: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let (seen, received) = mpsc::channel();
        let (release_slow, gate) = mpsc::channel();
        let block_next = Arc::new(AtomicBool::new(block_first));
        let pending_block = block_next.clone();
        let gate = Arc::new(std::sync::Mutex::new(gate));
        let stopped = Arc::new(AtomicBool::new(false));
        let stop = stopped.clone();
        let thread = thread::spawn(move || {
            let mut handlers = Vec::new();
            while !stop.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let seen = seen.clone();
                        let gate = gate.clone();
                        let should_block = pending_block.swap(false, Ordering::AcqRel);
                        handlers.push(thread::spawn(move || {
                            // Windows 接受的连接可能继承非阻塞状态；正文读取依赖下方超时。
                            stream.set_nonblocking(false).unwrap();
                            stream.set_read_timeout(Some(WAIT)).unwrap();
                            stream.set_write_timeout(Some(WAIT)).unwrap();
                            let body = read_body(&stream).unwrap();
                            let _ = seen.send(body.clone());
                            if should_block {
                                let _ = gate.lock().unwrap().recv_timeout(WAIT + WAIT);
                            }
                            let english = if body.contains(LATEST) { "I want water." } else { "I will eat first." };
                            let content = serde_json::json!({
                                "words": [], "sentence": null, "english_sentence": english
                            }).to_string();
                            let response = serde_json::json!({
                                "id": "local-test", "object": "chat.completion",
                                "created": 0, "model": "local-test",
                                "choices": [{"index": 0, "finish_reason": "stop",
                                    "message": {"role": "assistant", "content": content}}]
                            }).to_string();
                            // 请求被取消后连接可能已关闭，写入失败是本测试允许的正常结果。
                            let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", response.len(), response);
                        }));
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5))
                    }
                    Err(error) => panic!("本地 HTTP 服务接受连接失败: {error}"),
                }
            }
            for handler in handlers {
                let _ = handler.join();
            }
        });
        Self {
            address,
            received,
            release_slow,
            block_next,
            stopped,
            thread: Some(thread),
        }
    }

    fn predictor(&self, debounce_ms: u64) -> CloudPredictor {
        CloudPredictor::new(&PredictConfig {
            enabled: true,
            base_url: format!("http://{}", self.address),
            model: "local-test".into(),
            api_key: Some("local-test-key".into()),
            timeout_ms: 30_000,
            debounce_ms,
            sentence: false,
            ..PredictConfig::default()
        })
        .unwrap()
    }
}

impl Drop for MockServer {
    fn drop(&mut self) {
        let _ = self.release_slow.send(());
        self.stopped.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn read_body(stream: &TcpStream) -> io::Result<String> {
    let mut reader = BufReader::new(stream);
    let mut length = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "HTTP 请求头不完整",
            ));
        }
        if line == "\r\n" {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            length = value.trim().parse::<usize>().ok();
        }
    }
    let length = length
        .filter(|length| *length <= 65_536)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "HTTP 正文长度无效"))?;
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    String::from_utf8(body).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}
