//! 仅回放当前 Alt 组合键所需的瞬时原始字段；不累计历史。

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct InputKey {
    pub vk: u16,

    pub scan: u16,

    pub extended: bool,

    pub down: bool,
}

impl InputKey {
    pub fn release(self) -> Self {
        Self {
            down: false,
            ..self
        }
    }
}
