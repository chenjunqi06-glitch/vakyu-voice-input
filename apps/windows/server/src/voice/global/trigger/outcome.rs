//! 状态机输出；回放与事件发送在退出状态借用后执行。

use super::{event::TriggerEvent, input::InputKey};

#[derive(Default)]
pub(super) struct Outcome {
    pub consume: bool,

    /// 有当前组合键时，与此 Alt 按下一次性注入，保证先后顺序。
    pub replay_alt: Option<InputKey>,

    /// 短按回放 Alt 按下和抬起；组合键只回放按下。
    pub replay_release: bool,

    pub event: Option<TriggerEvent>,
}
