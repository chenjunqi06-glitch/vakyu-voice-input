# Vakyu macOS 原生语音宿主

这个菜单栏程序在 macOS 13 及以上独立运行。无需重启聊天软件，也无需切换到 Vakyu 输入源，即可使用两个独立入口：**左 Option 是普通语音输入**，松开后直接填入原话；**右 Control＋右 Option 是口语跟读**，启用跟读快捷键后，完整跟读或手动放行才填入原话。关闭跟读后，右侧组合不会退化为普通录音。两种方式都不会自动发送。

源码已实现双向跟读、完整字标半外露的紫色浮窗、实时麦克风电平、逐词过渡、中文拼音、离线词卡、本机朗读、重试/跳词/手动放行、菜单栏开关与语音设置。模型和学习规则由共享 Rust 服务处理，Swift 只负责 macOS 系统接口与界面。

## 构建与资源

在 Mac 上安装 Xcode Command Line Tools 后执行：

```sh
swift test --package-path apps/macos/companion --jobs 2
swift build --package-path apps/macos/companion -c release --arch arm64 --jobs 2
```

可执行文件为 `.build/arm64-apple-macosx/release/VakyuVoice`（使用 `swift build --show-bin-path` 获取实际目录）。外层打包脚本生成 `Vakyu.app/Contents/Resources/VakyuVoice.app`，语音宿主 Resources 包含：

- `vakyu-icon.png` 与 `vakyu-logo.png`；
- `vakyu-voice-service`；
- `voice-runtime/python/bin/python3`；
- `voice-runtime/app/external_main.py` 与 ASR 支持模块；
- `voice-runtime/models/`。

首次运行要在语音设置中允许麦克风、辅助功能与输入监控。设置保存到现有共享 `~/Library/Application Support/Qingjian/config.toml`；这个兼容路径不影响显示品牌。朗读音色是本机偏好 `voice.englishIdentifier` 和 `voice.chineseIdentifier`，不会调用云端音色服务。

## 系统集成约定

- `--settings` 冷启动或应用 reopen 打开语音设置。
- `app.vakyu.voice.open-settings` 分布式通知打开语音设置。
- 打开主输入法使用父 `.app --preferences`，同时发送 `app.vakyu.inputmethod.open-settings`。
- 服务协议见 `apps/voice-service/PROTOCOL.md`；每次 `press` 明确发送 `mode=dictation` 或 `mode=practice`，本轮模式锁定到完全松开。stdout 为 JSON 行，不保存录音、原文或密钥。
- `AVAudioEngine` 输出本机 16 kHz Float32LE PCM。音频转换串行；松开按键先排空已经采集的音频与重采样余量再送 `release`，取消直接使旧代次失效。
- 窗口使用非激活 NSPanel，固定本轮显示器的可见区域底部并避开 Dock。内外点击不关闭，也不把焦点移到浮窗。
- 每次填入验证原 PID、AX 输入元素、窗口、草稿内容及选区，拒绝密码和安全输入。先 AX 插入，再尝试受控 Cmd+V；不生成 Enter。不能确认粘贴成功时保留提示，禁止盲目重复粘贴。
- 快捷键保存前用 Carbon 尝试注册，失败不请求写配置。纯修饰键不能用 Carbon 检测；应用内部监听、硬件功能键的占用也不能完整检测。系统权限与应用 AX 实现仍需真机验收。
- `voice_key` 控制普通入口，默认 `left_alt`（左 Option），兼容已有 `alt`/`right_alt`；`voice_practice_shortcut` 只控制跟读入口。设置中快捷键录入与恢复默认都只修改跟读键；不能保存与普通入口相同的键。按 Option＋字母执行其他输入时不启动语音；同时按两入口或中途增加修饰键会中止这次手势，必须完全松开才能新开一轮。
- 旧 `alt` 设置在右侧组合跟读开启时，左 Option 用于普通语音，右 Option 留给跟读前缀，两种按下顺序都支持。显式 `right_alt` 与右侧组合有前缀冲突时会提示并禁用跟读入口，普通语音仍可用，不能把相互重叠的两种入口保存为新配置。

## 验证状态

当前开发主机为 Windows，未在此环境假称通过 Swift 编译或 macOS 应用实测。Swift 单元测试覆盖按键规范化、普通输入保护、跨语言快照解码、原草稿保护与 UTF-16 选区；必须在 Mac 执行上述命令。发布前还需要真实验收右侧组合键、千字长录音、两种跟读方向、词卡、Esc、焦点切换、微信/Codex/Muse 填入、全屏与多显示器，以及权限被拒绝后的恢复。
