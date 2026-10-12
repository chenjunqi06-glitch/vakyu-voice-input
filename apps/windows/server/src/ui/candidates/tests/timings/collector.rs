//! 聚合生产代码已有的耗时超过 5ms 的分段事件。

use tracing::{Event, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;

use super::StageSamples;
use super::fields::DurationFields;
pub(in crate::ui::candidates::tests) struct SlowStages(
    pub(in crate::ui::candidates::tests) StageSamples,
);

impl<S: Subscriber> Layer<S> for SlowStages {
    fn on_event(&self, event: &Event<'_>, _context: Context<'_, S>) {
        let mut visitor = DurationFields::default();
        event.record(&mut visitor);
        let stage = match visitor.message.as_str() {
            "候选窗渲染慢帧" => "render",
            "候选窗位图转换慢帧" => "dib_conversion",
            "分层窗口提交慢帧" => "update_layered_window",
            _ => return,
        };
        if let Some(milliseconds) = visitor.milliseconds {
            self.0
                .lock()
                .unwrap()
                .entry(stage.to_owned())
                .or_default()
                .push(milliseconds);
        }
    }
}
