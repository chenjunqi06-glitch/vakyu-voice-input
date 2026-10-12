//! 语音菜单请求打开输入法设置；分布式通知只允许这个固定动作。

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_foundation::{
    NSDistributedNotificationCenter, NSNotificationSuspensionBehavior, NSObject, NSObjectProtocol,
    ns_string,
};

define_class!(
    // SAFETY: NSObject 无额外子类要求；观察者在主事件循环内一直保留。
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ()]
    pub struct SettingsObserver;

    impl SettingsObserver {
        #[unsafe(method(openSettings:))]
        fn open_settings(&self, _notification: Option<&AnyObject>) {
            crate::host::with(|host| {
                host.settings.reload();
                host.apply_config(false);
                host.perform(crate::menubar::MenuAction::OpenPreferences);
            });
        }
    }

    unsafe impl NSObjectProtocol for SettingsObserver {}
);

impl SettingsObserver {
    pub fn register(mtm: MainThreadMarker) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(());
        let this: Retained<Self> = unsafe { msg_send![super(this), init] };
        // SAFETY: selector 与上面的回调一致；通知仅在注册的主线程投递。
        unsafe {
            NSDistributedNotificationCenter::defaultCenter()
                .addObserver_selector_name_object_suspensionBehavior(
                    &this,
                    sel!(openSettings:),
                    Some(ns_string!("app.vakyu.inputmethod.open-settings")),
                    None,
                    NSNotificationSuspensionBehavior::DeliverImmediately,
                );
        }
        this
    }
}
