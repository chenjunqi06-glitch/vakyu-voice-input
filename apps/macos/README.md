# Vakyu for macOS（Apple Silicon）

面向 M1 / M2 / M3 / M4 等 Apple 芯片，最低 macOS 13。输入法使用 InputMethodKit；全局语音独立常驻，可在其他输入源下使用。语音、双向跟读、词卡、渐进难度共享现有 Windows 核心。当前移植源码尚需 Mac 上编译和真人验收，不能把 Windows 上的检查当作 Mac 使用验证。

## 功能与使用

- 紫色渐变、白色文字、完整 Vakyu 标志；候选窗字标下部外露，语音跟读窗字标左上半外露。
- 普通语音默认按住左 Option 说话，松开直接填入；右 Control＋右 Option 是独立跟读入口。浮窗在当前屏幕下方、Dock 上方。设置里可修改跟读键。
- “启用跟读”默认关闭，开启后跟读键才生效，普通语音键始终直接填入。“学习英语”开＝说中文、读英文、填原中文；关＝说英文、读中文、填原英文。中文配拼音和英文释义。
- 词语点击可查本地词典和播放系统发音；可重试、跳词、手动放行。点击窗口内外不关闭，Esc 取消；成功填入后自动关闭。
- 仅填入原输入框，不模拟发送。目标内容或光标变动时保留原稿并提示，不写入其他窗口。
- 语音识别使用 CPU 两线程；词卡、本机发音不产生 API token。关闭云联想后，仅跟读的一次整句翻译请求使用云服务。

## 在 Apple 芯片 Mac 上构建

需要 Xcode（含 Swift 5.9+ 和 macOS SDK）、Rust 1.96+、Python 3.9+。在仓库根目录执行：

```bash
rustup target add aarch64-apple-darwin
bash tools/release/data-fetch.sh
CARGO_BUILD_JOBS=2 QINGJIAN_TARGET=aarch64-apple-darwin bash apps/macos/scripts/bundle.sh --pkg
```

构建脚本下载带 SHA256 校验的 ARM64 Python、识别依赖与中英流式语音模型；不会复制 Windows 的 DLL 或任何用户配置。产品词库下载失败时应处理网络问题，不要用少量样例词库替代完整交付。

输出 `target/Vakyu.app` 和 `target/pkg/vakyu-<版本>-macos-arm64.pkg`。`.github/workflows/macos-port.yml` 提供手动触发的原生构建和检查；没有执行过的 CI 不能算构建成功。签名与公证可通过 `QINGJIAN_SIGN_IDENTITY`、`QINGJIAN_INSTALLER_IDENTITY`、`QINGJIAN_NOTARY_PROFILE` 配置；未配置时是本地开发签名。

只安装到当前 Mac 用户：

```bash
CARGO_BUILD_JOBS=2 bash apps/macos/scripts/bundle.sh --install
```

系统输入源完成注册后，在输入源菜单选择 Vakyu。安装器另外创建用户登录项 `app.vakyu.voice`，使全局语音独立启动。输入源注册被系统延迟时可在系统键盘设置添加 Vakyu；语音权限不依赖输入源注册。

## 首次权限与设置

首次从菜单栏 Vakyu 图标进入语音设置，允许麦克风、辅助功能和输入监控。辅助功能用于定位/填入原输入框，输入监控用于全局按住快捷键。系统密码框、安全输入和拒绝辅助功能的应用不支持自动填入，不能承诺绕过这些系统限制。

普通按键组合用系统注册检测冲突，失败保留原键；右 Control＋右 Option 等纯修饰键无法被系统独占注册，其他软件仅在内部拦截的快捷键也无法完全检测。

翻译接口、模型和密钥在输入法“云服务”页设置，关闭云联想仍可编辑。旧数据目录 `~/Library/Application Support/Qingjian/` 保留兼容，避免丢词库；用户可见名称统一为 Vakyu。密钥保存在此目录 `.env`，不进入安装包或协议消息。普通输入法日志在 `~/Library/Logs/Qingjian/`；语音服务不记录录音或句子正文。

## 验证与卸载

功能对照与真人验收清单见 `docs/notes/macos-vakyu-port.md`，用户操作见 `docs/user/getting-started/macos-voice.md`。共享核心/服务测试可以在 Windows 跑；麦克风、权限、按键、微信和其他应用填入必须在 Mac 真机逐项验证。

```bash
bash /Library/Input\ Methods/Vakyu.app/Contents/Resources/uninstall.sh
```

默认卸载保留学习资料；显式 `--purge` 才清除兼容数据目录与日志。卸载也清除全局语音登录项。
