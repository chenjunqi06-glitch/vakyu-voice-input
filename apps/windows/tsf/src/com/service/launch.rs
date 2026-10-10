//! 连不上 Server 时自己拉起它。
//!
//! Server 只在登录时由「启动」文件夹的快捷方式拉起（Explorer 走 `ShellExecute`，见 `miracle.iss`），
//! 中途挂了（崩溃 / 被杀 / 装完没重启）以前 DLL 只能静默吞键到下次登录。这里在连接失败后起一次与
//! DLL 同目录的 `qingjian-server.exe`：用 `ShellExecuteExW` 而不是 `CreateProcess`——`uiAccess=true`
//! 的 exe 用 `CreateProcess` 拉不起来（报 740），ShellExecute 等同双击，两种构建都行。
//!
//! 两道闸门防重复启动：进程内的冷却时间（同一应用连敲只试一次），与跨进程的命名互斥体
//! （多个应用同时发现 Server 不在，只起一个）。后台线程在管道出现前保留互斥体，启动不阻塞输入线程。
//!
//! 另一道门是宿主的完整性级别：输入法会被加载进登录界面（LogonUI）、UAC 的 consent.exe 这类
//! 以 SYSTEM 跑在安全桌面上的进程，也会进 AppContainer 的商店应用（Low）。在这些宿主里
//! `ShellExecute` 要么起出一个 SYSTEM 权限、挂在安全桌面上的 Server，要么干脆起不来——
//! 只在普通桌面应用（Medium）里拉，其余照旧退避重连。
//!
//! 安装 / 卸载程序运行期间也不拉：它刚结束 Server 要替换 exe，这时拉起会占住文件（错误代码 5），
//! 安装程序持有命名互斥体 [`INSTALLER_MUTEX`]（见 `miracle.iss`），装完它自己起新 Server。

use std::ffi::c_void;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{
    CloseHandle, ERROR_ACCESS_DENIED, ERROR_ALREADY_EXISTS, ERROR_FILE_NOT_FOUND,
    ERROR_PATH_NOT_FOUND, FreeLibrary, GetLastError, HANDLE, HMODULE, WAIT_OBJECT_0,
};
use windows::Win32::Security::{
    GetSidSubAuthority, GetSidSubAuthorityCount, GetTokenInformation, TOKEN_MANDATORY_LABEL,
    TOKEN_QUERY, TokenIntegrityLevel,
};
use windows::Win32::System::Com::{
    COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx, CoUninitialize,
};
use windows::Win32::System::LibraryLoader::{
    FreeLibraryAndExitThread, GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GetModuleHandleExW,
};
use windows::Win32::System::Pipes::{NMPWAIT_NOWAIT, WaitNamedPipeW};
use windows::Win32::System::Threading::{
    CreateMutexW, CreateThread, GetCurrentProcess, OpenMutexW, OpenProcessToken,
    SYNCHRONIZATION_SYNCHRONIZE, THREAD_CREATION_FLAGS, WaitForSingleObject,
};
use windows::Win32::UI::Shell::{
    SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
    ShellExecuteExW,
};
use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;
use windows::core::{HSTRING, PCWSTR, w};

use qingjian_platform::protocol::DEFAULT_PIPE_NAME;

use crate::com::log::log;
use crate::com::module_path;

/// 同一进程内两次尝试拉起 Server 的最短间隔：连接失败每个键都会走到这里，别把应用砸出一串 Server。
const LAUNCH_COOLDOWN: Duration = Duration::from_secs(5);

/// 后台启动最久保留闸门的时间；失败或卡住后仍允许其他应用重试。
const STARTUP_GRACE: Duration = Duration::from_secs(15);

/// 跨进程互斥体：直到新进程建立管道、退出或超过启动宽限期才释放。
const LAUNCH_MUTEX: windows::core::PCWSTR = w!("Local\\QingjianServerLaunch");

/// 安装 / 卸载程序运行期间持有的互斥体，名字与 `miracle.iss` 的 `HoldInstallerMutex` 一致。
const INSTALLER_MUTEX: windows::core::PCWSTR = w!("Global\\QingjianInstaller");

/// 普通桌面应用的完整性级别 RID（UAC 未提升的用户进程）。低一档是 AppContainer / 浏览器沙箱，
/// 高一档是管理员提升、SYSTEM 与安全桌面上的进程——那些里都不拉 Server。
const MEDIUM_INTEGRITY_RID: u32 = 0x2000;

/// 上次尝试拉起的时间；本进程内所有文本服务实例共用。
static LAST_LAUNCH: Mutex<Option<Instant>> = Mutex::new(None);

/// 连不上 Server 时异步拉起它。已排入后台启动返回 `true`，没试 / 排入失败返回 `false`。
pub(super) fn launch_server() -> bool {
    if !cooldown_passed() {
        return false;
    }
    if !host_is_plain_desktop_app() {
        log("宿主进程不是普通桌面应用（完整性级别非 Medium），不拉起 Server");
        return false;
    }
    if installer_running() {
        log("安装程序正在运行，不拉起 Server");
        return false;
    }
    let Some(exe) = server_exe() else {
        log("找不到与 DLL 同目录的 qingjian-server.exe，不拉起");
        return false;
    };
    let Some(gate) = launch_mutex(LAUNCH_MUTEX) else {
        // 别的进程正在起：不重复起，退回退避重连
        return false;
    };
    if server_pipe_exists() {
        // 管道已被 Server 预留时，它可能还在初始化，或连接暂时已满。
        return false;
    }
    let mut module = HMODULE::default();
    // TSF 可在用户切换输入法时被 COM 卸载。单独持有模块引用，直到工作线程原子卸载并退出。
    if unsafe {
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
            PCWSTR(launch_thread as *const () as *const u16),
            &mut module,
        )
    }
    .is_err()
    {
        return false;
    }
    let request = Box::into_raw(Box::new(LaunchRequest {
        exe,
        _gate: gate,
        module,
    }));
    match unsafe {
        CreateThread(
            None,
            0,
            Some(launch_thread),
            Some(request.cast()),
            THREAD_CREATION_FLAGS(0),
            None,
        )
    } {
        Ok(thread) => {
            let _ = unsafe { CloseHandle(thread) };
            true
        }
        Err(error) => {
            drop(unsafe { Box::from_raw(request) });
            let _ = unsafe { FreeLibrary(module) };
            log(&format!("创建 Server 后台启动线程失败: {error}"));
            false
        }
    }
}

/// 线程参数同时持有启动闸门；模块引用由线程退出或建线程失败的分支显式释放。
struct LaunchRequest {
    exe: PathBuf,

    _gate: OwnedHandle,

    module: HMODULE,
}

unsafe extern "system" fn launch_thread(context: *mut c_void) -> u32 {
    let request = unsafe { Box::from_raw(context.cast::<LaunchRequest>()) };
    let module = request.module;
    let result = std::panic::catch_unwind(|| launch_in_background(&request.exe));
    if result.is_err() {
        log("Server 后台启动线程异常，已释放启动闸门");
    }
    drop(result);
    drop(request);
    // 不让卸载与线程最后一条指令之间留下执行已释放 DLL 代码的竞态。
    unsafe { FreeLibraryAndExitThread(module, 0) }
}

fn launch_in_background(exe: &Path) {
    // 等待线程调度期间安装器或另一个启动来源可能已经抢先完成。
    if installer_running() || server_pipe_exists() {
        return;
    }
    let initialized =
        unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) };
    if let Err(error) = initialized.ok() {
        log(&format!("初始化 Server 后台启动 COM 失败: {error}"));
        return;
    }
    let file = HSTRING::from(exe.as_os_str());
    let workdir = exe
        .parent()
        .map(|dir| HSTRING::from(dir.as_os_str()))
        .unwrap_or_default();
    // NOASYNC 只让后台线程等待 shell 完成启动；不让它退出后仍悬着 shell 的异步 COM 操作。
    let mut info = SHELLEXECUTEINFOW {
        cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_FLAG_NO_UI | SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC,
        lpVerb: w!("open"),
        lpFile: PCWSTR(file.as_ptr()),
        lpDirectory: PCWSTR(workdir.as_ptr()),
        nShow: SW_HIDE.0,
        ..Default::default()
    };
    let launched = unsafe { ShellExecuteExW(&mut info) };
    unsafe { CoUninitialize() };
    // x86 的 SHELLEXECUTEINFOW 为 packed，先复制字段再访问，避免未对齐引用。
    let process = info.hProcess;
    let process =
        (!process.is_invalid()).then(|| unsafe { OwnedHandle::from_raw_handle(process.0) });
    if let Err(error) = launched {
        log(&format!("静默启动 qingjian-server 失败: {error}"));
        return;
    }
    log("已在后台请求启动 qingjian-server");
    let started = Instant::now();
    while started.elapsed() < STARTUP_GRACE {
        if server_pipe_exists()
            || process.as_ref().is_some_and(|process| unsafe {
                WaitForSingleObject(HANDLE(process.as_raw_handle()), 0) == WAIT_OBJECT_0
            })
        {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// 只查询管道，不连入或发送消息；忙碌和拒绝访问也表明 Server 已持有单实例锁。
fn server_pipe_exists() -> bool {
    let name = HSTRING::from(DEFAULT_PIPE_NAME);
    if unsafe { WaitNamedPipeW(&name, NMPWAIT_NOWAIT) }.as_bool() {
        return true;
    }
    !matches!(
        unsafe { GetLastError() },
        ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND
    )
}

/// 进程内冷却：距上次尝试不到 [`LAUNCH_COOLDOWN`] 就不试。时间在尝试**之前**记，失败也冷却。
fn cooldown_passed() -> bool {
    let Ok(mut last) = LAST_LAUNCH.lock() else {
        return false;
    };
    if last.is_some_and(|at| at.elapsed() < LAUNCH_COOLDOWN) {
        return false;
    }
    *last = Some(Instant::now());
    true
}

/// 宿主进程是不是普通桌面应用（完整性级别为 Medium）。
///
/// 读当前进程令牌的 `TokenIntegrityLevel`，SID 最后一个子授权值就是级别 RID。
/// 读不到一律当成「不是」：宁可不拉（退回退避重连，下次登录时由启动文件夹的快捷方式起），
/// 也不能在安全桌面上起出一个 SYSTEM 权限的 Server。
fn host_is_plain_desktop_app() -> bool {
    let mut token = HANDLE::default();
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) }.is_err() {
        return false;
    }
    let mut needed = 0u32;
    // 两段式：第一次只问要多少字节（总是报缓冲区不够，属正常）。
    unsafe {
        let _ = GetTokenInformation(token, TokenIntegrityLevel, None, 0, &mut needed);
    }
    let mut buffer = vec![0u8; needed as usize];
    let queried = needed > 0
        && unsafe {
            GetTokenInformation(
                token,
                TokenIntegrityLevel,
                Some(buffer.as_mut_ptr().cast()),
                needed,
                &mut needed,
            )
        }
        .is_ok();
    let _ = unsafe { CloseHandle(token) };
    if !queried {
        return false;
    }
    let label = unsafe {
        buffer
            .as_ptr()
            .cast::<TOKEN_MANDATORY_LABEL>()
            .read_unaligned()
    };
    let sid = label.Label.Sid;
    let count_ptr = unsafe { GetSidSubAuthorityCount(sid) };
    if count_ptr.is_null() {
        return false;
    }
    let count = unsafe { *count_ptr };
    if count == 0 {
        return false;
    }
    let rid_ptr = unsafe { GetSidSubAuthority(sid, u32::from(count) - 1) };
    !rid_ptr.is_null() && unsafe { *rid_ptr } == MEDIUM_INTEGRITY_RID
}

/// 安装程序的互斥体在不在。它由提升的安装程序建，普通应用打开会被拒（`ERROR_ACCESS_DENIED`），拒也说明存在。
fn installer_running() -> bool {
    match unsafe { OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, false, INSTALLER_MUTEX) } {
        Ok(handle) => {
            let _ = unsafe { CloseHandle(handle) };
            true
        }
        Err(error) => error.code() == ERROR_ACCESS_DENIED.to_hresult(),
    }
}

/// 跨进程互斥体；已被别的进程持有（对方正在起 Server）返回 `None`。
fn launch_mutex(name: PCWSTR) -> Option<OwnedHandle> {
    let handle = unsafe { CreateMutexW(None, false, name) }.ok()?;
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        let _ = unsafe { CloseHandle(handle) };
        return None;
    }
    Some(unsafe { OwnedHandle::from_raw_handle(handle.0) })
}

fn server_exe() -> Option<PathBuf> {
    let module = module_path().ok()?;
    Some(server_exe_path(Path::new(&module.to_string())))
}

/// 与 DLL 同目录的 `qingjian-server.exe`（安装器把两者装在同一目录）。
fn server_exe_path(module: &Path) -> PathBuf {
    module.with_file_name("qingjian-server.exe")
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use windows::core::{HSTRING, PCWSTR};

    use super::{launch_mutex, server_exe_path};

    #[test]
    fn server_exe_sits_next_to_the_dll() {
        assert_eq!(
            server_exe_path(Path::new(
                r"D:\Program Files\Qingjian\qingjian_tsf-0.1.0-alpha.15-dev.dll"
            )),
            Path::new(r"D:\Program Files\Qingjian\qingjian-server.exe")
        );
    }

    #[test]
    fn launch_gate_stays_exclusive_until_background_owner_exits() {
        let name = HSTRING::from(format!("Local\\VakyuLaunchTest-{}", std::process::id()));
        let key = PCWSTR(name.as_ptr());
        let first = launch_mutex(key).expect("首次恢复应取得闸门");
        assert!(launch_mutex(key).is_none(), "后台启动期间不能再次恢复");
        drop(first);
        assert!(launch_mutex(key).is_some(), "后台启动结束后允许恢复");
    }
}
