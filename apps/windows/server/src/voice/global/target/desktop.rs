//! 仅在本工作线程所属的交互桌面上输入；锁屏、UAC 或无权查询时立即拒绝。

use std::mem::size_of_val;

use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::StationsAndDesktops::{
    CloseDesktop, DESKTOP_CONTROL_FLAGS, DESKTOP_READOBJECTS, GetThreadDesktop,
    GetUserObjectInformationW, HDESK, OpenInputDesktop, UOI_NAME,
};
use windows::Win32::System::Threading::GetCurrentThreadId;

use super::error::TargetError;

struct InputDesktop(HDESK);

impl Drop for InputDesktop {
    fn drop(&mut self) {
        let _ = unsafe { CloseDesktop(self.0) };
    }
}

pub(super) fn validate_input_desktop() -> Result<(), TargetError> {
    // 不申请切换、建窗或钩子权限；失败关闭，不去打开安全桌面或切换线程桌面。
    let input = unsafe { OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, DESKTOP_READOBJECTS) }
        .map(InputDesktop)
        .map_err(|_| TargetError::DesktopChanged)?;
    let thread = unsafe { GetThreadDesktop(GetCurrentThreadId()) }
        .map_err(|_| TargetError::DesktopChanged)?;
    let input_name = desktop_name(input.0)?;
    let thread_name = desktop_name(thread)?;
    if !desktop_names_match(&input_name, &thread_name) {
        return Err(TargetError::DesktopChanged);
    }
    Ok(())
}

pub(super) fn desktop_names_match(input: &[u16], thread: &[u16]) -> bool {
    !input.is_empty() && !input.contains(&0) && input == thread
}

fn desktop_name(desktop: HDESK) -> Result<Vec<u16>, TargetError> {
    let mut name = [0u16; 256];
    let mut needed = 0u32;
    unsafe {
        GetUserObjectInformationW(
            HANDLE(desktop.0),
            UOI_NAME,
            Some(name.as_mut_ptr().cast()),
            size_of_val(&name) as u32,
            Some(&mut needed),
        )
    }
    .map_err(|_| TargetError::DesktopChanged)?;
    let units = (needed as usize) / 2;
    if !needed.is_multiple_of(2) || units < 2 || units > name.len() || name[units - 1] != 0 {
        return Err(TargetError::DesktopChanged);
    }
    Ok(name[..units - 1].to_vec())
}
