//! 将 tracing 事件的 Duration 调试字段转换成毫秒。

use tracing::field::{Field, Visit};
#[derive(Default)]
pub(super) struct DurationFields {
    pub(super) message: String,

    pub(super) milliseconds: Option<f64>,
}

impl Visit for DurationFields {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        let value = format!("{value:?}");
        match field.name() {
            "message" => self.message = value.trim_matches('"').to_owned(),
            "elapsed" => {
                self.milliseconds = [("ms", 1.0), ("µs", 0.001), ("ns", 0.000_001), ("s", 1000.0)]
                    .into_iter()
                    .find_map(|(unit, scale)| {
                        value
                            .strip_suffix(unit)?
                            .parse::<f64>()
                            .ok()
                            .map(|number| number * scale)
                    });
            }
            _ => {}
        }
    }
}
