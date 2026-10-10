//! 线程焦点通知：本线程得到键盘焦点时 service 向 Server 取一次全局中英模式（别的应用里可能刚切过）。
//! 切窗口时 `ITfKeyEventSink::OnSetFocus` 不触发，跟着窗口焦点走的是这条。

use windows::Win32::UI::TextServices::{
    ITfSource, ITfThreadFocusSink, ITfThreadFocusSink_Impl, ITfThreadMgr,
};
use windows::core::{Interface, Result, implement};

#[implement(ITfThreadFocusSink)]
struct ThreadFocusSink;

impl ITfThreadFocusSink_Impl for ThreadFocusSink_Impl {
    fn OnSetThreadFocus(&self) -> Result<()> {
        super::service::on_thread_focus(true);
        Ok(())
    }

    fn OnKillThreadFocus(&self) -> Result<()> {
        super::service::on_thread_focus(false);
        Ok(())
    }
}

/// 挂上通知，返回 `(source, cookie)` 供 [`unadvise`]。
pub(super) fn advise(thread_mgr: &ITfThreadMgr) -> Result<(ITfSource, u32)> {
    let source: ITfSource = thread_mgr.cast()?;
    let sink: ITfThreadFocusSink = ThreadFocusSink.into();
    let cookie = unsafe { source.AdviseSink(&ITfThreadFocusSink::IID, &sink)? };
    Ok((source, cookie))
}

pub(super) fn unadvise(source: &ITfSource, cookie: u32) {
    let _ = unsafe { source.UnadviseSink(cookie) };
}
