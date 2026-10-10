/// 这台机器要哪个安装包：索引里资产的 `platform` 与 `cpu`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Target {
    pub platform: &'static str,

    pub cpu: &'static str,
}

impl Target {
    /// 按编译目标取。Windows 只有 x86_64 的包（arm64 机器上也是它）。
    pub fn current() -> Self {
        let platform = if cfg!(target_os = "macos") {
            "macos"
        } else if cfg!(windows) {
            "windows"
        } else {
            "linux"
        };
        let cpu = if cfg!(all(target_arch = "aarch64", not(windows))) {
            "arm64"
        } else {
            "x86_64"
        };
        Self { platform, cpu }
    }
}
