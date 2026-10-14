//! 捕获并保留目标进程句柄，提前辨别 UIPI 可能拒绝的高权限窗口。

use std::mem::size_of;

use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Security::{
    GetSidSubAuthority, GetSidSubAuthorityCount, GetTokenInformation, TOKEN_MANDATORY_LABEL,
    TOKEN_QUERY, TokenIntegrityLevel, TokenUIAccess,
};
use windows::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
};

use super::error::TargetError;

pub(super) struct ProcessAccess(HANDLE);

impl ProcessAccess {
    pub(super) fn capture(pid: u32) -> Result<Self, TargetError> {
        let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }
            .map(Self)
            .map_err(|_| TargetError::Permission)?;
        let (current_integrity, ui_access) = token_rights(unsafe { GetCurrentProcess() })?;
        let (target_integrity, _) = token_rights(process.0)?;
        if !integrity_allows(current_integrity, target_integrity, ui_access) {
            return Err(TargetError::Permission);
        }
        Ok(process)
    }
}

impl Drop for ProcessAccess {
    fn drop(&mut self) {
        let _ = unsafe { CloseHandle(self.0) };
    }
}

pub(super) fn integrity_allows(current: u32, target: u32, ui_access: bool) -> bool {
    // UIAccess 可跨普通提升边界，但不能借此进入 SYSTEM 等安全界面。
    target < 0x4000 && (target <= current || ui_access)
}

fn token_rights(process: HANDLE) -> Result<(u32, bool), TargetError> {
    let mut raw_token = HANDLE::default();
    unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut raw_token) }
        .map_err(|_| TargetError::Permission)?;
    let token = ProcessAccess(raw_token);
    let mut needed = 0;
    let _ = unsafe { GetTokenInformation(token.0, TokenIntegrityLevel, None, 0, &mut needed) };
    if needed < size_of::<TOKEN_MANDATORY_LABEL>() as u32 || needed > 65536 {
        return Err(TargetError::Permission);
    }
    // usize 保证 TOKEN_MANDATORY_LABEL 对齐，SID 指针只在此缓冲区存活期间使用。
    let mut buffer = vec![0usize; (needed as usize).div_ceil(size_of::<usize>())];
    unsafe {
        GetTokenInformation(
            token.0,
            TokenIntegrityLevel,
            Some(buffer.as_mut_ptr().cast()),
            needed,
            &mut needed,
        )
    }
    .map_err(|_| TargetError::Permission)?;
    let label = unsafe { &*buffer.as_ptr().cast::<TOKEN_MANDATORY_LABEL>() };
    let count = unsafe { GetSidSubAuthorityCount(label.Label.Sid).as_ref() }
        .copied()
        .filter(|count| *count > 0)
        .ok_or(TargetError::Permission)?;
    let integrity = unsafe { GetSidSubAuthority(label.Label.Sid, u32::from(count) - 1).as_ref() }
        .copied()
        .ok_or(TargetError::Permission)?;
    let mut ui_access = 0u32;
    unsafe {
        GetTokenInformation(
            token.0,
            TokenUIAccess,
            Some((&mut ui_access as *mut u32).cast()),
            size_of::<u32>() as u32,
            &mut needed,
        )
    }
    .map_err(|_| TargetError::Permission)?;
    Ok((integrity, ui_access != 0))
}
