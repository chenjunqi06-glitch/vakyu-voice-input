# Vakyu macOS 原生语音宿主

这个菜单栏程序在 macOS 13 及以上独立运行。无需重启聊天软件，也无需切换到 Vakyu 输入源，即可使用两个独立入口：**左 Option 是普通语音输入**，松开后直接填入原话；**右 Control＋右 Option 是口语跟读**，启用跟读快捷键后，完整跟读或手动放行才填入原话。关闭跟读后，右侧组合不会退化为普通录音。两种方式都不会自动发送。

源码已实现双向跟读、完整字标半外露的紫色浮窗、实时麦克风电平、逐词过渡、中文拼音、离线词卡、本机朗读、重试/跳词/手动放行、菜单栏开关与语音设置。模型和学习规则由共享 Rust 服务处理，Swift 只负责 macOS 系统接口与界面。

## 构建与资源

在 Mac 上安装并首次打开完整 Xcode 后执行（Command Line Tools 可编译宿主，但不包含单元测试需要的 XCTest）：

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

首次运行按语音设置顶部的引导允许麦克风与辅助功能；快捷键仍无法连接时，页面才会提供输入监控入口。设置保存到现有共享 `~/Library/Application Support/Qingjian/config.toml`；这个兼容路径不影响显示品牌。朗读音色是本机偏好 `voice.englishIdentifier` 和 `voice.chineseIdentifier`，不会调用云端音色服务。

## 系统集成约定

- `--settings` 冷启动或应用 reopen 打开语音设置。
- `app.vakyu.voice.open-settings` 分布式通知打开语音设置。
- 打开主输入法使用父 `.app --preferences`，同时发送 `app.vakyu.inputmethod.open-settings`。
- 服务协议见 `apps/voice-service/PROTOCOL.md`；每次 `press` 明确发送 `mode=dictation` 或 `mode=practice`，本轮模式锁定到完全松开。stdout 为 JSON 行，不保存录音、原文或密钥。
- `AVAudioEngine` 输出本机 16 kHz Float32LE PCM。音频转换串行；松开按键先排空已经采集的音频与重采样余量再送 `release`，取消直接使旧代次失效。
- 窗口使用非激活 NSPanel，固定本轮显示器的可见区域底部并避开 Dock。内外点击不关闭，也不把焦点移到浮窗。
- 可读取的输入框继续核对原 PID、AX 元素、窗口、草稿和选区。读不到输入框时，可使用设置中的“无法读取输入框时直接粘贴”（默认开启）；仅在相同前台应用与窗口、无安全输入且焦点不是密码框时尝试一次 Cmd+V，不生成 Enter。结果显示“已粘贴，无法核对”，保留复制入口，禁止重复粘贴。关闭开关后，已经录音但尚未投递的兜底目标也不再自动粘贴。
- 快捷键保存前用 Carbon 尝试注册，失败不请求写配置。纯修饰键不能用 Carbon 检测；应用内部监听、硬件功能键的占用也不能完整检测。系统权限与应用 AX 实现仍需真机验收。
- `voice_key` 控制普通入口，默认 `left_alt`（左 Option），兼容已有 `alt`/`right_alt`；`voice_practice_shortcut` 只控制跟读入口。设置中快捷键录入与恢复默认都只修改跟读键；不能保存与普通入口相同的键。按 Option＋字母执行其他输入时不启动语音；同时按两入口或中途增加修饰键会中止这次手势，必须完全松开才能新开一轮。
- 旧 `alt` 设置在右侧组合跟读开启时，左 Option 用于普通语音，右 Option 留给跟读前缀，两种按下顺序都支持。显式 `right_alt` 与右侧组合有前缀冲突时会提示并禁用跟读入口，普通语音仍可用，不能把相互重叠的两种入口保存为新配置。
- 右 Command＋右 Option 可录入为跟读键，规范配置值为 `right_win+right_alt`；`right_cmd` / `right_command` 是输入别名。录入器在全松键后确认，错误组合不会在松键时退化成单 Option。
- 麦克风用途说明同时写入外层 IMK app 和嵌套语音 app；macOS TCC 可能将麦克风请求归属外层，缺少任一声明都不允许交付安装包。

## 验证状态

已在 Apple Silicon、macOS 26.4.1、Swift 6.3 完成 debug/release 原生编译。29 项 XCTest 因当前机器缺少完整 Xcode 而尚未执行；本机纯 Swift 键盘回放的 140 条断言通过。Swift 单元测试覆盖按键规范化、普通输入保护、跨语言快照解码、原草稿保护与 UTF-16 选区。发布前还需要真实验收右侧组合键、千字长录音、两种跟读方向、词卡、Esc、焦点切换、微信/Codex/Muse 填入、全屏与多显示器，以及权限被拒绝后的恢复。完整记录见 `docs/notes/macos-native-validation.md`。
