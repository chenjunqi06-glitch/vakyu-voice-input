//! 一次听写绑定的输入位置与存活句柄，不保存输入框原有内容。

use std::rc::Rc;

use qingjian_platform::protocol::ScreenRect;

use super::access::ProcessAccess;
use super::apartment::Apartment;
use super::native::NativeFocus;
use super::proof::EditProof;

pub(crate) struct LockedTarget {
    pub(super) native: NativeFocus,

    pub(super) proof: EditProof,

    /// 持有进程句柄，避免录音期间 PID 被退出后的新进程复用。
    pub(super) _process: ProcessAccess,

    pub(super) rect: ScreenRect,

    /// 此字段最后释放，COM 元素代理必须先析构。
    pub(super) _apartment: Rc<Apartment>,
}

impl LockedTarget {
    pub(crate) fn rect(&self) -> ScreenRect {
        self.rect
    }

    pub(crate) fn process_id(&self) -> u32 {
        self.native.focus_pid
    }

    pub(crate) fn thread_id(&self) -> u32 {
        self.native.thread
    }

    pub(crate) fn delivery_method(&self) -> &'static str {
        match &self.proof {
            EditProof::TextService(_) => "tsf",
            EditProof::Native => "native_unicode",
            EditProof::Automation { .. } => "uia_unicode",
        }
    }
}
