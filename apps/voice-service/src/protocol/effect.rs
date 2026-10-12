//! 控制器副作用，测试可直接检查且不启动麦克风或网络。

use super::Event;

pub enum Effect {
    Native(Event),

    Asr(serde_json::Value),
}
