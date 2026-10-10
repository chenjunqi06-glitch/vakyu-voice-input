//! 只测试启用策略与身份，不调用会修改 Windows 用户输入列表的系统接口。

use super::{CLSID, INSTALL_FLAGS, PROFILE, PROFILE_SPEC, same_parent_directory, should_enable};

#[test]
fn only_first_login_or_explicit_install_enables_profile() {
    assert!(should_enable(false, false));
    assert!(should_enable(true, false));
    assert!(should_enable(true, true));
    // 用户后来从 Windows 设置移除输入法，普通启动不能再次强行添加。
    assert!(!should_enable(false, true));
}

#[test]
fn installer_flags_never_change_default_or_remove_other_profiles() {
    assert_eq!(INSTALL_FLAGS, 0);
    assert_eq!(PROFILE_SPEC, format!("0x0804:{{{CLSID:?}}}{{{PROFILE:?}}}"));
    assert!(!PROFILE_SPEC.contains(';'));
}

#[test]
fn development_executable_cannot_enroll_another_installation() {
    let folder = std::env::temp_dir().join(format!("vakyu-profile-paths-{}", std::process::id()));
    std::fs::create_dir_all(folder.join("installed")).unwrap();
    std::fs::create_dir_all(folder.join("development")).unwrap();
    let installed = folder.join("installed/server.exe");
    let dll = folder.join("installed/profile.dll");
    assert!(same_parent_directory(&installed, &dll));
    assert!(!same_parent_directory(
        &folder.join("development/server.exe"),
        &dll
    ));
    std::fs::remove_dir_all(folder).unwrap();
}
