//! 当前登录用户的输入法启用；只添加本产品，不重置语言列表或默认输入法。

#[cfg(test)]
mod tests;

use std::mem::size_of;
use std::path::{Path, PathBuf};

use windows::Win32::Foundation::{CloseHandle, FreeLibrary, HANDLE};
use windows::Win32::Security::{GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoUninitialize,
};
use windows::Win32::System::LibraryLoader::{
    GetProcAddress, LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::Win32::UI::TextServices::{
    CLSID_TF_InputProcessorProfiles, ITfInputProcessorProfiles,
};
use windows::core::{BOOL, GUID, PCWSTR, s, w};

// 与 TSF 的 com/mod.rs 同一注册身份；保留 GUID 才能原位升级历史安装。
const CLSID: GUID = GUID::from_u128(0x4fdca82d_e923_49bf_9e75_bb906b93b8bb);
const PROFILE: GUID = GUID::from_u128(0x8119f8e0_cf81_423b_9189_c0d7374324b3);
const PROFILE_SPEC: &str =
    "0x0804:{4FDCA82D-E923-49BF-9E75-BB906B93B8BB}{8119F8E0-CF81-423B-9189-C0D7374324B3}";
const MACHINE_REGISTRATION: &str =
    r"SOFTWARE\Classes\CLSID\{4FDCA82D-E923-49BF-9E75-BB906B93B8BB}\InprocServer32";
const MARKER: &str = "input-profile-enabled-v1";

/// 官方 InstallLayoutOrTip：0 只添加指定项；不能用默认项、默认用户或清空列表标志。
const INSTALL_FLAGS: u32 = 0;

/// 普通启动每用户执行一次；安装器显式命令可修复用户原来移除的条目。
/// 构建或测试目录的 Server 不修改真实登录用户的输入列表。
pub fn enable_for_current_user(explicit_install: bool) -> bool {
    match enable(explicit_install) {
        Ok(()) => true,
        Err(message) => {
            tracing::warn!(%message, "Vakyu 当前用户输入法条目未能自动启用；可在 Windows 语言设置中添加 Vakyu，语音入口不受影响");
            false
        }
    }
}

fn enable(explicit_install: bool) -> Result<(), String> {
    if !belongs_to_registered_installation() {
        tracing::debug!("不在已注册安装目录，跳过用户输入法启用");
        return Ok(());
    }
    // UAC 使用另一个管理员的凭据时，不能把当前用户的条目加给那个管理员。
    if !is_standard_user_process() {
        return Err("请从普通桌面打开 Vakyu，或下次正常登录时自动完成启用".to_owned());
    }
    let marker = marker_path().ok_or_else(|| "无法取得当前用户数据目录".to_owned())?;
    if !should_enable(explicit_install, marker.is_file()) {
        return Ok(());
    }
    let initialized = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok();
    let result = enable_in_apartment();
    if initialized {
        unsafe { CoUninitialize() };
    }
    result?;
    std::fs::create_dir_all(marker.parent().expect("用户标记父目录"))
        .and_then(|()| std::fs::write(&marker, PROFILE_SPEC))
        .map_err(|_| "输入法已启用，但首次启用标记未能保存".to_owned())?;
    tracing::info!("已为当前用户启用 Vakyu 输入法；默认输入法与其他键盘保持原样");
    Ok(())
}

fn enable_in_apartment() -> Result<(), String> {
    let profiles: ITfInputProcessorProfiles =
        unsafe { CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER) }
            .map_err(|_| "Windows 文本服务当前不可用".to_owned())?;
    // 机器级 TSF 注册不等于当前用户的切换列表已有条目；首次安装仍向 input.dll 添加一次。
    install_current_user_profile()?;
    if !unsafe { profiles.IsEnabledLanguageProfile(&CLSID, 0x0804, &PROFILE) }
        .is_ok_and(|enabled| enabled.as_bool())
    {
        return Err("Windows 尚未确认 Vakyu 条目；下次登录会重新尝试".to_owned());
    }
    Ok(())
}

fn install_current_user_profile() -> Result<(), String> {
    // 此 API 不带导入库；限制 System32 搜索，不能让工作目录提供同名 input.dll。
    // https://learn.microsoft.com/windows/win32/tsf/installlayoutortip
    let library = unsafe { LoadLibraryExW(w!("input.dll"), None, LOAD_LIBRARY_SEARCH_SYSTEM32) }
        .map_err(|_| "无法加载 Windows 输入法管理组件".to_owned())?;
    let result = match unsafe { GetProcAddress(library, s!("InstallLayoutOrTip")) } {
        Some(address) => {
            let install: unsafe extern "system" fn(PCWSTR, u32) -> BOOL =
                unsafe { std::mem::transmute(address) };
            let profile: Vec<u16> = PROFILE_SPEC.encode_utf16().chain(Some(0)).collect();
            if unsafe { install(PCWSTR(profile.as_ptr()), INSTALL_FLAGS) }.as_bool() {
                Ok(())
            } else {
                Err("Windows 拒绝启用 Vakyu 输入法条目".to_owned())
            }
        }
        None => Err("Windows 缺少输入法启用接口".to_owned()),
    };
    let _ = unsafe { FreeLibrary(library) };
    result
}

fn marker_path() -> Option<PathBuf> {
    Some(
        PathBuf::from(std::env::var_os("LOCALAPPDATA")?)
            .join("Qingjian")
            .join(MARKER),
    )
}

fn should_enable(explicit_install: bool, marker_exists: bool) -> bool {
    explicit_install || !marker_exists
}

fn belongs_to_registered_installation() -> bool {
    let Ok(executable) = std::env::current_exe() else {
        return false;
    };
    let registered = windows_registry::LOCAL_MACHINE
        .open(MACHINE_REGISTRATION)
        .and_then(|key| key.get_string(""));
    registered.is_ok_and(|dll| same_parent_directory(&executable, Path::new(&dll)))
}

fn same_parent_directory(executable: &Path, dll: &Path) -> bool {
    match (
        executable
            .parent()
            .and_then(|path| path.canonicalize().ok()),
        dll.parent().and_then(|path| path.canonicalize().ok()),
    ) {
        (Some(executable), Some(dll)) => executable
            .to_string_lossy()
            .eq_ignore_ascii_case(&dll.to_string_lossy()),
        _ => false,
    }
}

fn is_standard_user_process() -> bool {
    let mut token = HANDLE::default();
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) }.is_err() {
        return false;
    }
    let mut elevation = TOKEN_ELEVATION::default();
    let mut returned = 0;
    let result = unsafe {
        GetTokenInformation(
            token,
            TokenElevation,
            Some((&mut elevation as *mut TOKEN_ELEVATION).cast()),
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut returned,
        )
    };
    let _ = unsafe { CloseHandle(token) };
    result.is_ok() && elevation.TokenIsElevated == 0
}
