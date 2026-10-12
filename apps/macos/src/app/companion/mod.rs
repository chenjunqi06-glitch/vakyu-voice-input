//! 独立语音程序与 IMK 设置之间的入口；不传递语音正文或配置凭据。

mod observer;

pub use observer::SettingsObserver;

/// 打开语音设置。`open` 复用已经运行的实例，登录项仍负责后台常驻。
pub fn open_settings() -> Result<(), String> {
    let path = super::paths::resources_dir()
        .map_err(|error| error.to_string())?
        .join("VakyuVoice.app");
    if !path.is_dir() {
        return Err("语音组件未安装，请安装包含语音组件的 Vakyu 完整版。".into());
    }
    std::process::Command::new("/usr/bin/open")
        .arg("-a")
        .arg(path)
        .args(["--args", "--settings"])
        .spawn()
        .map_err(|error| format!("无法打开语音设置：{error}"))?;
    Ok(())
}
