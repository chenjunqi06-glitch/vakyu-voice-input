//! 反向同步：监听「转换模式」（用户点任务栏中 / 英）与「输入法开 / 关」（系统 Ctrl + Space）两条 compartment，
//! 变了让 service 跟着改中英模式。回调里只读不写；自己写触发的那次读回与当前相同，service 里忽略。

use windows::Win32::UI::TextServices::{
    GUID_COMPARTMENT_KEYBOARD_INPUTMODE_CONVERSION, GUID_COMPARTMENT_KEYBOARD_OPENCLOSE,
    ITfCompartment, ITfCompartmentEventSink, ITfCompartmentEventSink_Impl, ITfSource, ITfThreadMgr,
};
use windows::core::{GUID, Interface, Result, implement};

use crate::com::log::log;
use crate::com::service;

#[implement(ITfCompartmentEventSink)]
struct SystemModeSink;

impl ITfCompartmentEventSink_Impl for SystemModeSink_Impl {
    fn OnChange(&self, rguid: *const GUID) -> Result<()> {
        let guid = unsafe { *rguid };
        if guid == GUID_COMPARTMENT_KEYBOARD_INPUTMODE_CONVERSION {
            service::on_conversion_mode_changed();
        } else if guid == GUID_COMPARTMENT_KEYBOARD_OPENCLOSE {
            service::on_keyboard_open_changed();
        }
        Ok(())
    }
}

/// 挂上的回调（source + cookie），停用时 [`unadvise`]。
pub(crate) type Advice = Vec<(ITfSource, u32)>;

/// 两条 compartment 各挂一个回调；一条挂不上只记日志，另一条照常。
pub(crate) fn advise(thread_mgr: &ITfThreadMgr) -> Advice {
    let sink: ITfCompartmentEventSink = SystemModeSink.into();
    let targets = [
        ("转换模式", super::conversion_compartment(thread_mgr)),
        ("输入法开 / 关", super::openclose_compartment(thread_mgr)),
    ];
    targets
        .into_iter()
        .filter_map(|(what, compartment)| {
            compartment
                .and_then(|compartment| advise_one(&compartment, &sink))
                .map_err(|error| log(&format!("监听{what}失败: {error}")))
                .ok()
        })
        .collect()
}

fn advise_one(
    compartment: &ITfCompartment,
    sink: &ITfCompartmentEventSink,
) -> Result<(ITfSource, u32)> {
    let source: ITfSource = compartment.cast()?;
    let cookie = unsafe { source.AdviseSink(&ITfCompartmentEventSink::IID, sink)? };
    Ok((source, cookie))
}

pub(crate) fn unadvise(advice: Advice) {
    for (source, cookie) in advice {
        let _ = unsafe { source.UnadviseSink(cookie) };
    }
}
