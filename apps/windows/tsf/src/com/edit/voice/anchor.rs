//! 语音开始时的文档锚点及异步读写状态；取消后排队中的编辑回调也不会写入。

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::TextServices::{ITfContext, ITfRange, ITfThreadMgr};
use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

pub(crate) struct VoiceAnchor {
    pub(super) context: ITfContext,

    manager: ITfThreadMgr,

    window: HWND,

    pub(super) range: RefCell<Option<ITfRange>>,

    pub(super) ready: Cell<bool>,

    pub(super) cancelled: Cell<bool>,

    pub(super) queued: Cell<bool>,

    pub(super) committed: Cell<bool>,

    deadline_ms: Cell<Option<u64>>,
}

impl VoiceAnchor {
    pub(crate) fn new(context: ITfContext, manager: ITfThreadMgr) -> Rc<Self> {
        Rc::new(Self {
            context,
            manager,
            window: unsafe { GetForegroundWindow() },
            range: RefCell::new(None),
            ready: Cell::new(false),
            cancelled: Cell::new(false),
            queued: Cell::new(false),
            committed: Cell::new(false),
            deadline_ms: Cell::new(None),
        })
    }

    pub(crate) fn focused(&self) -> bool {
        !self.cancelled.get()
            && self.deadline_ms.get().is_none_or(|deadline| {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .is_ok_and(|now| now.as_millis() < u128::from(deadline))
            })
            && !self.window.is_invalid()
            && unsafe { GetForegroundWindow() } == self.window
            && unsafe { self.manager.IsThreadFocus() }.is_ok_and(|focus| focus.as_bool())
            && !crate::com::context::keyboard_disabled(&self.context)
            && unsafe {
                self.manager
                    .GetFocus()
                    .and_then(|document| document.GetTop())
            }
            .is_ok_and(|context| context == self.context)
    }

    pub(crate) fn ready(&self) -> bool {
        self.ready.get()
    }

    pub(crate) fn queued(&self) -> bool {
        self.queued.get()
    }

    pub(crate) fn committed(&self) -> bool {
        self.committed.get()
    }

    pub(crate) fn cancel(&self) {
        self.cancelled.set(true);
    }

    pub(crate) fn cancelled(&self) -> bool {
        self.cancelled.get()
    }

    pub(crate) fn set_deadline(&self, deadline_ms: u64) {
        self.deadline_ms.set(Some(deadline_ms));
    }
}
