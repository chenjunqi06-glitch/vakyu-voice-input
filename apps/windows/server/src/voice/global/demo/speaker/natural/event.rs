//! 朗读进程发回的一行事件；协议见 `voice-runtime/speak_main.py`。

#[derive(Debug, PartialEq)]
pub(super) enum Event {
    Ready,

    Done(u32),

    Failed(u32, String),

    /// 模型或声卡组件加载失败，进程随后退出。
    Fatal(String),
}

impl Event {
    pub fn parse(line: &str) -> Option<Self> {
        let value: serde_json::Value = serde_json::from_str(line).ok()?;
        let id = || {
            value
                .get("id")
                .and_then(serde_json::Value::as_u64)
                .and_then(|id| u32::try_from(id).ok())
        };
        let message = |fallback: &str| {
            value
                .get("message")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(fallback)
                .to_owned()
        };
        match value.get("event")?.as_str()? {
            "ready" => Some(Self::Ready),
            "done" => Some(Self::Done(id()?)),
            "error" => Some(Self::Failed(id()?, message("示范朗读失败，可继续跟读"))),
            "fatal" => Some(Self::Fatal(message("自然人声组件无法加载"))),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Event;

    #[test]
    fn parses_each_event_and_ignores_garbage() {
        assert_eq!(Event::parse(r#"{"event":"ready"}"#), Some(Event::Ready));
        assert_eq!(
            Event::parse(r#"{"event":"done","id":3}"#),
            Some(Event::Done(3))
        );
        assert_eq!(
            Event::parse(r#"{"event":"error","id":4,"message":"无法播放"}"#),
            Some(Event::Failed(4, "无法播放".to_owned()))
        );
        assert_eq!(
            Event::parse(r#"{"event":"fatal"}"#),
            Some(Event::Fatal("自然人声组件无法加载".to_owned()))
        );
        assert_eq!(Event::parse(r#"{"event":"done"}"#), None);
        assert_eq!(Event::parse(r#"{"event":"done","id":99999999999}"#), None);
        assert_eq!(Event::parse("garbage"), None);
    }
}
