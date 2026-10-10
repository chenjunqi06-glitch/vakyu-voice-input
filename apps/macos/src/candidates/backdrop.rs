//! 候选窗底板和完整字标，字标底部三分之一露在圆角框之外。

use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSCompositingOperation, NSImage, NSView};
use objc2_foundation::{NSObjectProtocol, NSPoint, NSRect, NSSize};

pub const MARK_HEIGHT: f64 = 32.0;

define_class!(
    // SAFETY: NSView 允许子类化；图片在主线程使用。
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[ivars = Option<Retained<NSImage>>]
    pub struct CandidateBackdrop;

    impl CandidateBackdrop {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let bounds = self.bounds();
            let outside = MARK_HEIGHT / 3.0;
            crate::app::brand::background(NSRect::new(
                NSPoint::new(0.0, outside),
                NSSize::new(bounds.size.width, (bounds.size.height - outside).max(0.0)),
            ), 8.0);
            let Some(image) = self.ivars() else { return; };
            let source = image.size();
            let height = MARK_HEIGHT.min((bounds.size.width - 16.0).max(1.0) * source.height / source.width.max(1.0));
            let rect = NSRect::new(NSPoint::new(8.0, 0.0), NSSize::new(height * source.width / source.height.max(1.0), height));
            // SAFETY: 当前 drawRect 有有效图形上下文；从完整 PNG 按比例绘制。
            unsafe {
                image.drawInRect_fromRect_operation_fraction_respectFlipped_hints(
                    rect, NSRect::ZERO, NSCompositingOperation::SourceOver, 1.0, true, None,
                );
            }
        }
    }

    unsafe impl NSObjectProtocol for CandidateBackdrop {}
);

impl CandidateBackdrop {
    pub fn new(mtm: MainThreadMarker, view: &NSView) -> Retained<Self> {
        let this = mtm
            .alloc::<Self>()
            .set_ivars(crate::app::brand::image("vakyu-logo.png"));
        let this: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: NSRect::ZERO] };
        this.addSubview(view);
        this
    }
}
