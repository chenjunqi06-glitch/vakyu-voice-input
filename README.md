<p align="center"><img src="assets/icon/windows/vakyu-icon.png" alt="Vakyu" height="96"></p>

# Vakyu · macOS

支持普通中文输入、语音转文字和英语／中文跟读的输入法。Mac 移植保留现有输入引擎、界面、词库和学习功能。

## 使用

- 普通语音：按住左 Option 说话，松开后填入原输入框。
- 口语跟读：默认右 Control＋右 Option；可在语音设置修改，已有快捷键会保留。
- 支持英语／中文双向跟读、词卡、系统朗读和本机语音识别。跟读翻译使用你自行配置的服务。
- 只填入文字，不自动发送消息或回车。
- 部分应用读不到输入框时，可使用“无法读取输入框时直接粘贴”；它只在原应用、原窗口且无安全输入时尝试一次，无法核对时保留复制文字入口。

## 当前 Mac 测试版本

[下载 macOS 构建 9](https://github.com/chenjunqi06-glitch/vakyu-voice-input/releases/tag/macos-build9-20261013) · [Windows 版本](https://github.com/chenjunqi06-glitch/vakyu-voice-input/tree/main)

适用于 Apple Silicon Mac，最低系统要求 macOS 13。本测试版基于本机已安装并验收的 **构建 9 / 0.1.5-dev-unknown**。

构建 9 使用临时开发签名，**尚未经过 Apple Developer ID 签名与公证**。首次安装可能需要在系统设置中确认打开，更新后可能需要重新添加辅助功能权限。不要关闭系统安全功能。麦克风、辅助功能等系统授权需要使用者本人确认。

使用说明：[Mac 语音输入](docs/user/getting-started/macos-voice.md)。构建说明：[Mac 构建与安装](apps/macos/scripts/README.md)。验证范围：[构建 9 验收记录](docs/notes/macos-build9-validation.md)。

## 从源码构建

安装 Rust、完整 Xcode 和 Python 后，在仓库根目录运行：

```sh
rustup target add aarch64-apple-darwin
bash tools/release/data-fetch.sh
QINGJIAN_TARGET=aarch64-apple-darwin bash apps/macos/scripts/bundle.sh --pkg
```

词库与模型按现有清单获取，不把本机用户配置或 API 密钥放入仓库。首次授权和不同应用的输入兼容性仍需在实际机器上验证。

## 项目与许可

基于现有 Qingjian／Miracle 源码继续开发 Mac 版本，保留各平台源码及原许可。原项目说明见 [upstream-readme.md](docs/notes/upstream-readme.md)。代码许可为 [GPL-3.0-or-later](LICENSE)，第三方数据的来源及许可见 [数据来源清单](docs/design/landscape.md)。

此分支是已验收的 macOS 构建 9 源码快照，原始移交包不含 Git 历史。Windows 继续使用仓库的 `main` 分支及原有 Release，本分支不覆盖它们。构建工作流保存在 `docs/github-workflows/macos-build9/` 作为参考，不自动执行。

感谢青简原作者及主要维护者 [@yenharvey](https://github.com/yenharvey)、qingjian-team 与所有贡献者。Vakyu 是独立衍生项目，不是青简官方发行版。详见 [上游与许可说明](UPSTREAM.md)。
