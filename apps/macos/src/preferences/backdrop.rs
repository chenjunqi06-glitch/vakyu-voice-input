//! 设置页的品牌渐变底板；原生控件仍保留键盘和辅助功能支持。

use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::NSView;
use objc2_foundation::{NSObjectProtocol, NSRect};

define_class!(
    // SAFETY: NSView 无额外子类化要求。
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ()]
    pub struct Backdrop;

    impl Backdrop {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            crate::app::brand::background(self.bounds(), 12.0);
        }
    }

    unsafe impl NSObjectProtocol for Backdrop {}
);

impl Backdrop {
    pub fn new(mtm: MainThreadMarker, rect: NSRect) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(());
        unsafe { msg_send![super(this), initWithFrame: rect] }
    }
}
