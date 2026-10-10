//! 用独立管道验证启动门闸，不接触安装中的输入法和用户配置。

use super::reserve_pipe;
use windows::Win32::Foundation::ERROR_ACCESS_DENIED;

#[test]
fn reservation_rejects_duplicates_and_releases_on_exit() {
    let name = format!(r"\\.\pipe\vakyu-startup-test-{}", std::process::id());
    let first = reserve_pipe(&name).unwrap();
    let error = reserve_pipe(&name).unwrap_err();
    assert_eq!(error.raw_os_error(), Some(ERROR_ACCESS_DENIED.0 as i32));
    drop(first);
    assert!(reserve_pipe(&name).is_ok());
}
