//! 计时汇总与现有慢帧事件采集，不逐帧打印日志。

mod collector;
mod fields;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

pub(super) use collector::SlowStages;
pub(super) type StageSamples = Arc<Mutex<BTreeMap<String, Vec<f64>>>>;
pub(super) fn stats(samples: &[f64]) -> serde_json::Value {
    if samples.is_empty() {
        return serde_json::json!({"count": 0});
    }
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    serde_json::json!({
        "count": samples.len(),
        "average_ms": samples.iter().sum::<f64>() / samples.len() as f64,
        "p95_ms": sorted[(samples.len() * 95).div_ceil(100) - 1],
        "max_ms": sorted[samples.len() - 1],
    })
}
