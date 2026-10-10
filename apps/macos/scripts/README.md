# Mac 构建与安装

完整 Vakyu 语音版在 Apple 芯片 Mac 构建，目标系统为 macOS 13 及以上。构建机需要完整 Xcode 15 或以上（Swift 5.9+、Metal 工具链）、Rust 1.96、Python 3.9+、网络和约 5 GB 临时空间。Swift、sips、iconutil、codesign、pkgbuild 和 productbuild 使用系统工具。构建默认两个并行任务，避免占满较低配置机器。

```sh
rustup target add aarch64-apple-darwin
CARGO_BUILD_JOBS=2 QINGJIAN_TARGET=aarch64-apple-darwin bash apps/macos/scripts/bundle.sh --pkg
```

产物 `target/Vakyu.app` 和 `target/pkg/vakyu-<版本>-macos-arm64.pkg`。打包同时编译 IMK、跨平台语音服务和 Swift 宿主；运行时下载来源与 SHA256 在 `../voice-runtime/sources.json` 固定。完整包包含 CPU 语音模型、离线词典、图标和完整字标。下载缓存只在 `target/macos-download-cache`，不会读取本机用户配置或将 API 密钥打包。

默认开发包使用 ad-hoc 签名。对外发行要设置 `QINGJIAN_SIGN_IDENTITY`（Developer ID Application）、`QINGJIAN_INSTALLER_IDENTITY`（Developer ID Installer）；可另设 `QINGJIAN_NOTARY_PROFILE` 自动提交公证并 staple。签名顺序是 Python 动态库和其他 Mach-O、带 audio-input entitlement 的语音 app、最外层 IMK app；最后执行深度验证。没有 Apple 证书或公证不能声称正式发行通过。

`--install` 安装到当前用户 `~/Library/Input Methods/Vakyu.app`；pkg 安装到 `/Library/Input Methods/Vakyu.app`。请只保留一种安装位置，避免相同输入源 ID 重复。系统包升级仅在确认 bundle ID 后清理旧名称 Qingjian.app，用户配置和学习经验保持原位置。

登录用户会安装 `~/Library/LaunchAgents/app.vakyu.voice.plist`，启动独立 `VakyuVoice.app`，从而已有软件和未来安装的软件均可由全局语音宿主处理。安装时没有登录用户，或只复制 `.app` 的情况，可以运行：

```sh
sh '/Library/Input Methods/Vakyu.app/Contents/Resources/install_voice_agent.sh' '/Library/Input Methods/Vakyu.app'
```

首次使用必须由用户在 macOS 授予麦克风、辅助功能和输入监控。密码字段、安全输入、受保护系统窗口等仍受系统限制。首次安装输入法若列表未刷新，系统可能要求注销；语音宿主不依赖其他应用重启。Mac 右 Option 对应 Windows 右 Alt，右 Control＋右 Option 可配置为跟读组合键。

卸载使用 app 内 `Resources/uninstall.sh`，会停止语音登录项并删除程序；只有 `--purge` 才删除配置、学习资料、日志。不要用 sudo 运行整个卸载脚本，脚本需要时单独请求系统安装目录权限。

`VAKYU_WITH_VOICE=0` 仅用于维护旧版纯 IMK 构建（包括 Intel），不属于本次完整 Apple Silicon 交付。Windows 可运行 Python 协议测试和清单检查，但不能编译 Swift/AppKit/IMK，也不能代替 Mac 麦克风、快捷键、焦点及自动填入的实际验收。
