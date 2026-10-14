//! 全局听写的目标锁定：只向仍获焦、已证实可编辑且非密码的控件送入文字。

mod access;
mod apartment;
mod context;
mod desktop;
mod editable;
mod error;
mod input;
mod locked;
mod native;
mod proof;

pub(super) use context::TargetContext;
pub(super) use locked::LockedTarget;

/// 让低级键盘钩子识别本产品送入的 Unicode 事件，避免触发或取消另一轮录音。
pub(crate) const GLOBAL_VOICE_INPUT_TAG: usize = 0x514A_564F;

#[cfg(test)]
mod tests;
