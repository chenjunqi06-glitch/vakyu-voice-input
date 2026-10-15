//! 在语音原插入点写入最终文本；新拼音尚在组句时延后，避免语音被并入组句后覆盖。

use std::mem::ManuallyDrop;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use windows::Win32::Foundation::E_FAIL;
use windows::Win32::UI::TextServices::{
    INSERT_TEXT_AT_SELECTION_FLAGS, ITfContext, ITfEditSession, ITfEditSession_Impl,
    ITfInsertAtSelection, ITfRange, TF_AE_END, TF_ANCHOR_END, TF_ANCHOR_START,
    TF_DEFAULT_SELECTION, TF_ES_READWRITE, TF_GRAVITY_FORWARD, TF_SD_READONLY, TF_SELECTION,
    TF_SELECTIONSTYLE,
};
use windows::core::{Error, Interface, Result, implement};

use super::anchor::VoiceAnchor;
use crate::com::composition::Shared;
use crate::com::log::log;

#[implement(ITfEditSession)]
struct CommitSession {
    anchor: Rc<VoiceAnchor>,

    shared: Rc<Shared>,

    text: String,
}

impl ITfEditSession_Impl for CommitSession_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        if !self.anchor.submission.begin() {
            return Ok(());
        }
        let result = catch_unwind(AssertUnwindSafe(|| {
            if self.anchor.committed() || !self.anchor.focused() {
                self.anchor.cancel();
                return Ok(());
            }
            // 排队期间用户可能开始新组句；保留结果，下一拍在组句结束后重试。
            if self.shared.composing() || self.shared.has_composition() {
                return Ok(());
            }
            let range = self
                .anchor
                .range
                .borrow()
                .clone()
                .ok_or_else(|| Error::from(E_FAIL))?;
            // 只允许插入，宿主没有遵守 gravity 时绝不能把后来输入的文字替换掉。
            if !unsafe { range.IsEmpty(ec)? }.as_bool() {
                return Err(Error::from(E_FAIL));
            }
            let context = &self.anchor.context;
            if unsafe { context.GetStatus()? }.dwDynamicFlags & TF_SD_READONLY != 0
                || crate::com::edit::surrounding::private_input(context, ec, &range)
            {
                return Err(Error::from(E_FAIL));
            }
            super::diagnostic::context(context, "提交语音");
            let (selection, style) = saved_selection(context, ec)?;
            let follow_caret = unsafe {
                selection.IsEmpty(ec)?.as_bool()
                    && range.CompareStart(ec, &selection, TF_ANCHOR_START)? == 0
            };
            // 临时切选区只为走宿主的插入接口；保存的范围跟随文字位移，稍后恢复用户原选区。
            unsafe { selection.SetGravity(ec, TF_GRAVITY_FORWARD, TF_GRAVITY_FORWARD)? };
            let insert: ITfInsertAtSelection = context.cast()?;
            set_selection(context, ec, &range, collapsed_style())?;
            let text: Vec<u16> = self.text.encode_utf16().collect();
            // Win32 编辑框的过渡上下文允许 SetText 成功却不提交到正文；与普通无组句上屏共用宿主插入接口。
            let inserted = match unsafe {
                insert.InsertTextAtSelection(ec, INSERT_TEXT_AT_SELECTION_FLAGS(0), &text)
            } {
                Ok(inserted) => inserted,
                Err(error) => {
                    if let Err(restore) = set_selection(context, ec, &selection, style) {
                        log(&format!("语音插入失败后恢复选区失败: {restore}"));
                    }
                    return Err(error);
                }
            };
            // 插入接口已经执行，后续光标操作失败也不能重试正文，避免重复插入。
            self.anchor.submission.commit();
            super::diagnostic::inserted(&inserted, ec, text.len());
            let moved = if follow_caret {
                unsafe { inserted.Collapse(ec, TF_ANCHOR_END) }
                    .and_then(|()| set_selection(context, ec, &inserted, collapsed_style()))
            } else {
                set_selection(context, ec, &selection, style)
            };
            if let Err(error) = moved {
                log(&format!("宿主已接受语音插入，恢复光标或选区失败: {error}"));
            }
            log(&format!(
                "宿主已接受语音 InsertTextAtSelection 请求，跟随原光标={follow_caret}"
            ));
            Ok(())
        }));
        let result = match result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => {
                self.anchor.cancel();
                log(&format!("语音插入原位置失败: {error}"));
                Err(error)
            }
            Err(_) => {
                self.anchor.cancel();
                log("语音插入回调 panic（已兜住）");
                Err(Error::from(E_FAIL))
            }
        };
        // 写入和光标恢复均可能重入；直到整个回调结束才允许组句延后的请求重排。
        self.anchor.submission.finish();
        result
    }
}

fn saved_selection(context: &ITfContext, ec: u32) -> Result<(ITfRange, TF_SELECTIONSTYLE)> {
    let mut selections = [TF_SELECTION::default()];
    let mut fetched = 0;
    unsafe { context.GetSelection(ec, TF_DEFAULT_SELECTION, &mut selections, &mut fetched)? };
    let range = unsafe { ManuallyDrop::take(&mut selections[0].range) };
    if fetched == 0 {
        return Err(Error::from(E_FAIL));
    }
    let range = range.ok_or_else(|| Error::from(E_FAIL))?;
    Ok((unsafe { range.Clone()? }, selections[0].style))
}

fn collapsed_style() -> TF_SELECTIONSTYLE {
    TF_SELECTIONSTYLE {
        ase: TF_AE_END,
        fInterimChar: false.into(),
    }
}

fn set_selection(
    context: &ITfContext,
    ec: u32,
    range: &ITfRange,
    style: TF_SELECTIONSTYLE,
) -> Result<()> {
    let selection = TF_SELECTION {
        range: ManuallyDrop::new(Some(range.clone())),
        style,
    };
    let result = unsafe { context.SetSelection(ec, std::slice::from_ref(&selection)) };
    drop(ManuallyDrop::into_inner(selection.range));
    result
}

pub(crate) fn request_voice_commit(
    anchor: Rc<VoiceAnchor>,
    client_id: u32,
    shared: Rc<Shared>,
    text: String,
) -> Result<()> {
    if !anchor.submission.queue() {
        return Ok(());
    }
    let context = anchor.context.clone();
    let result = crate::com::edit::update::request(
        &context,
        client_id,
        CommitSession {
            anchor: anchor.clone(),
            shared,
            text,
        }
        .into(),
        TF_ES_READWRITE,
    );
    if result.is_err() {
        anchor.submission.finish();
        anchor.cancel();
    }
    result
}
