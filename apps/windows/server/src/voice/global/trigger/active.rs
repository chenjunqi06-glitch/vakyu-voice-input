//! 单次暂扣 Alt 的轮次状态。

use super::input::InputKey;
use std::time::Instant;

pub(super) struct Active {
    pub key: InputKey,

    pub sequence: u64,

    pub pressed: Instant,

    pub held: bool,
}
