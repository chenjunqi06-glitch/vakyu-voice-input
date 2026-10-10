//! SAPI 事件的所有权封装；GetEvents 交出的 COM 引用与缓冲必须逐一释放。

use windows::Win32::Media::Speech::{
    ISpRecoResult, SPET_LPARAM_IS_OBJECT, SPET_LPARAM_IS_POINTER, SPET_LPARAM_IS_STRING,
    SPET_LPARAM_IS_TOKEN, SPEVENT,
};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::core::{IUnknown, Interface, PWSTR};

pub(super) struct Event(pub SPEVENT);

impl Event {
    pub fn id(&self) -> i32 {
        self.0._bitfield & 0xffff
    }

    fn parameter_type(&self) -> i32 {
        (self.0._bitfield >> 16) & 0xffff
    }

    pub fn text(&self) -> windows::core::Result<String> {
        let raw = self.0.lParam.0 as *mut std::ffi::c_void;
        if raw.is_null() || self.parameter_type() != SPET_LPARAM_IS_OBJECT.0 {
            return Ok(String::new());
        }
        // 事件拥有原始引用；cast 取得独立引用，不能从原始指针再接管一次所有权。
        unsafe {
            let object = IUnknown::from_raw_borrowed(&raw).expect("非空事件指针");
            let result: ISpRecoResult = object.cast()?;
            let mut text = PWSTR::null();
            result.GetText(0, u32::MAX, true, &mut text, None)?;
            let decoded = if text.is_null() {
                Ok(String::new())
            } else {
                text.to_string()
            };
            CoTaskMemFree(Some(text.0.cast()));
            Ok(decoded?)
        }
    }
}

impl Drop for Event {
    fn drop(&mut self) {
        let raw = self.0.lParam.0 as *mut std::ffi::c_void;
        if raw.is_null() {
            return;
        }
        unsafe {
            match self.parameter_type() {
                kind if kind == SPET_LPARAM_IS_OBJECT.0 || kind == SPET_LPARAM_IS_TOKEN.0 => {
                    drop(IUnknown::from_raw(raw));
                }
                kind if kind == SPET_LPARAM_IS_POINTER.0 || kind == SPET_LPARAM_IS_STRING.0 => {
                    CoTaskMemFree(Some(raw));
                }
                _ => {}
            }
        }
    }
}
