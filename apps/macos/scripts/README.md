# Mac 构建与安装

完整 Vakyu 语音版在 Apple 芯片 Mac 构建，目标系统为 macOS 13 及以上。构建机需要完整 Xcode 15 或以上（Swift 5.9+、Metal 工具链）、Rust 1.96、Python 3.9+、网络和约 5 GB 临时空间。Swift、sips、iconutil、codesign、pkgbuild 和 productbuild 使用系统工具。构建默认两个并行任务，避免占满较低配置机器。

```sh
rustup target add aarch64-apple-darwin
CARGO_BUILD_JOBS=2 QINGJIAN_TARGET=aarch64-apple-darwin bash apps/macos/scripts/bundle.sh --pkg
```

产物 `target/macos-bundle.noindex/Vakyu.app` 和 `target/pkg/vakyu-<版本>-macos-arm64.pkg`。构建目录排除 Spotlight 索引；生成组件包后删除临时 payload app，避免构建副本与已安装输入法争用同一身份。打包同时编译 IMK、跨平台语音服务和 Swift 宿主；运行时下载来源与 SHA256 在 `../voice-runtime/sources.json` 固定。完整包包含 CPU 语音模型、离线词典、图标和完整字标。下载缓存只在 `target/macos-download-cache`，不会读取本机用户配置或将 API 密钥打包。

本机签名优先使用 `QINGJIAN_SIGN_IDENTITY`，其次 `QINGJIAN_LOCAL_SIGN_IDENTITY`；两者都未设置时，只在唯一候选时自动选择本机有效的 Apple Development 或名称为 `Vakyu Local Code Signing` 的固定代码签名证书。没有唯一固定身份才退回 ad-hoc，并显示更新可能导致权限失效的警告。后续构建应始终使用同一证书与 bundle ID；这能减少身份变化，但不代替用户首次授权。使用证书 SHA-1 指定正式发行身份时，可设 `QINGJIAN_SIGN_TIMESTAMP=1` 要求时间戳；Developer ID Application 名称默认开启时间戳。脚本不会创建证书或修改系统信任。对外发行要设置 `QINGJIAN_SIGN_IDENTITY`（Developer ID Application）、`QINGJIAN_INSTALLER_IDENTITY`（Developer ID Installer）；可另设 `QINGJIAN_NOTARY_PROFILE` 自动提交公证并 staple。签名顺序是 Python 动态库和其他 Mach-O、带 audio-input entitlement 的语音 app、最外层 IMK app；最后执行深度验证。没有 Apple 证书或公证不能声称正式发行通过。

`--install` 安装到当前用户 `~/Library/Input Methods/Vakyu.app`；pkg 安装到 `/Library/Input Methods/Vakyu.app`。两种安装方式发现另一位置已有同身份安装时会停止，不再只提醒后继续产生重复副本。升级保留原应用，完成最终路径登记后，由两个独立进程确认父输入源与模式存在、已启用、模式可选择、图标 URL 指向安装目录且用户/系统输入法安装目录中没有第二份；工作区里的构建产物不算已安装副本。短暂系统重扫允许重新登记。失败返回非零并恢复原安装，再登记原包。此核验确认安装/注册状态，不代表已经完成每个应用的真实打字验收。用户配置和学习资料保持原位置。

登录用户会安装 `~/Library/LaunchAgents/app.vakyu.voice.plist`，启动独立 `VakyuVoice.app`。系统安装器要求存在登录的桌面用户，才能完成输入源核验；没有登录用户时会在写入载荷之前停止。只复制 `.app` 的情况下，可在最终安装目录先运行 `Resources/register_input_source.sh`，再安装语音启动项：

```sh
sh '/Library/Input Methods/Vakyu.app/Contents/Resources/install_voice_agent.sh' '/Library/Input Methods/Vakyu.app'
```

首次使用由用户按语音设置页引导授予麦克风、辅助功能；已有辅助功能授权且快捷键监听正常时不再要求单独输入监控。密码字段、安全输入、受保护系统窗口等仍受系统限制。Mac 右 Option 对应 Windows 右 Alt，右侧修饰键可用于配置独立跟读组合。

卸载使用 app 内 `Resources/uninstall.sh`，会停止语音登录项并删除程序；只有 `--purge` 才删除配置、学习资料、日志。不要用 sudo 运行整个卸载脚本，脚本需要时单独请求系统安装目录权限。

`VAKYU_WITH_VOICE=0` 仅用于维护旧版纯 IMK 构建（包括 Intel），不属于本次完整 Apple Silicon 交付。Windows 可运行 Python 协议测试和清单检查，但不能编译 Swift/AppKit/IMK，也不能代替 Mac 麦克风、快捷键、焦点及自动填入的实际验收。
