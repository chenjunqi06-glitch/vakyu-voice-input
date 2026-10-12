//! 异步只读会话取得语音插入点；向后依附确保后续打字不会把锚点推到句尾。

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use windows::Win32::Foundation::E_FAIL;
use windows::Win32::UI::TextServices::{
    ITfEditSession, ITfEditSession_Impl, TF_ANCHOR_START, TF_ES_READ, TF_GRAVITY_BACKWARD,
    TF_SD_READONLY,
};
use windows::core::{Error, Result, implement};

use super::anchor::VoiceAnchor;
use crate::com::edit::anchor::{anchor_rect, selection_range};
use crate::com::edit::surrounding::private_input;
use crate::com::log::log;
use crate::com::service::SharedClient;

#[implement(ITfEditSession)]
struct CaptureSession {
    anchor: Rc<VoiceAnchor>,

    engine: SharedClient,
}

impl ITfEditSession_Impl for CaptureSession_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        let result = catch_unwind(AssertUnwindSafe(|| {
            if !self.anchor.focused() {
                log("语音插入点已失去焦点");
                return Err(Error::from(E_FAIL));
            }
            let range =
                selection_range(&self.anchor.context, ec).ok_or_else(|| Error::from(E_FAIL))?;
            if unsafe { self.anchor.context.GetStatus()? }.dwDynamicFlags & TF_SD_READONLY != 0
                || private_input(&self.anchor.context, ec, &range)
            {
                log("私密或只读输入框不启用语音");
                return Err(Error::from(E_FAIL));
            }
            unsafe {
                range.Collapse(ec, TF_ANCHOR_START)?;
                range.SetGravity(ec, TF_GRAVITY_BACKWARD, TF_GRAVITY_BACKWARD)?;
            }
            let rect = anchor_rect(&self.anchor.context, ec, &range);
            *self.anchor.range.borrow_mut() = Some(range);
            self.anchor.ready.set(true);
            super::diagnostic::context(&self.anchor.context, "保存插入点");
            log("语音插入点已就绪");
            if let Ok(mut guard) = self.engine.try_borrow_mut()
                && let Some(client) = guard.as_mut()
            {
                // 同一线程刚从私密输入框切过来时，Server 可能还保留旧标记。
                let _ = client.set_private(false);
                let _ = client.position_candidates(rect);
            }
            crate::com::service::on_voice_anchor_ready(&self.anchor);
            Ok(())
        }));
        match result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => {
                self.anchor.cancel();
                log(&format!("读取语音插入点失败: {error}"));
                Err(error)
            }
            Err(_) => {
                self.anchor.cancel();
                log("读取语音插入点回调 panic（已兜住）");
                Err(Error::from(E_FAIL))
            }
        }
    }
}

pub(crate) fn request_voice_anchor(
    anchor: Rc<VoiceAnchor>,
    client_id: u32,
    engine: SharedClient,
) -> Result<()> {
    let context = anchor.context.clone();
    crate::com::edit::update::request(
        &context,
        client_id,
        CaptureSession { anchor, engine }.into(),
        TF_ES_READ,
    )
}
