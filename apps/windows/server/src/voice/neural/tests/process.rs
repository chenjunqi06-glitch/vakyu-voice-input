//! 使用已安装的真实子进程验证预热管道；绝不发送 start，不访问麦克风。

use std::io::{BufReader, Write};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::voice::neural::event::HelperEvent;
use crate::voice::neural::process::HelperProcess;
use crate::voice::neural::process::read_event;

#[test]
fn ready_line_is_delivered_while_pipe_writer_remains_open() {
    let (reader, mut writer) = std::io::pipe().unwrap();
    let (sender, receiver) = mpsc::channel();
    let thread = std::thread::spawn(move || {
        let _ = sender.send(read_event(&mut BufReader::new(reader)));
    });
    writer.write_all(b"{\"event\":\"ready\"}\n").unwrap();
    let result = receiver.recv_timeout(Duration::from_secs(2));
    drop(writer);
    thread.join().unwrap();
    assert!(matches!(result, Ok(Ok(HelperEvent::Ready))));
}

#[test]
#[ignore = "需要安装中文神经网络语音组件；只预热，不开启麦克风"]
fn installed_helper_ready_then_shutdown_without_microphone() {
    let root = crate::voice::neural::runtime_root().expect("已安装语音 runtime");
    let started = Instant::now();
    let mut helper = HelperProcess::spawn(&root).expect("启动真实语音 helper");
    let result = helper.events.recv_timeout(Duration::from_secs(60));
    eprintln!("语音预热管道等待 {:?}，结果 {result:?}", started.elapsed());
    assert!(matches!(result, Ok(Ok(HelperEvent::Ready))));
    helper.send("shutdown", 0).expect("发送 shutdown");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if helper.check_alive().is_err() {
            break;
        }
        assert!(Instant::now() < deadline, "helper 未按 shutdown 正常退出");
        std::thread::sleep(Duration::from_millis(20));
    }
}
