<p align="center"><img src="assets/icon/windows/vakyu-icon.png" alt="Vakyu" height="96"></p>

# Vakyu · 本地语音输入与中英跟读

把日常表达变成口语练习：按住快捷键说话，松开转成文字；也可以先听译句、跟读完成，再把原文或译文填入原输入框。由你决定何时发送。

**普通语音、跟读翻译、划词翻译和示范朗读均可在本机完成，无需购买 API 额度或填写翻译密钥。** Windows 随包提供识别、翻译和自然人声模型；Mac 使用本机识别模型与 Apple 系统翻译、系统声音。首次准备系统语言或下载安装包需要联网，准备完成后这些功能可离线使用。

## 下载

| 平台 | 当前交付 | 要求 |
|---|---|---|
| Windows | [Preview 5 安装包与源码](https://github.com/chenjunqi06-glitch/vakyu-voice-input/releases/tag/v0.1.5-preview.5) | Windows 10 22H2 / Windows 11，Intel / AMD x64 |
| Mac 新功能版 | [本机翻译合并源码预览](https://github.com/chenjunqi06-glitch/vakyu-voice-input/releases/tag/macos-local-translation-20261015) | Apple Silicon；系统翻译需要 macOS 15+，先下载中英翻译语言；本次尚未生成或验收新 PKG |
| Mac 已发布安装包 | [构建 9 PKG](https://github.com/chenjunqi06-glitch/vakyu-voice-input/releases/tag/macos-build9-20261013) | Apple Silicon，macOS 13+；旧版本，不包含本次本机翻译改动 |

Windows 下载文件名以 `windows-x86_64-setup.exe` 结尾的附件，运行安装器，然后从开始菜单打开「Vakyu 设置」。完整安装包含词典、运行库和模型，无需另装 Python 或开发工具。拼音输入按 `Win + Space` 切换到 Vakyu；独立语音入口只需后台运行。

Mac 本次源码以已验收构建 9 为基础合入 Claude 的本机翻译与浮窗改动，保留微信等应用的受限粘贴、填入结果核对和权限提示。源码分支为 [macos-local-translation](https://github.com/chenjunqi06-glitch/vakyu-voice-input/tree/macos-local-translation)。旧构建 9 的 TextEdit、Safari、微信验收记录不能替代本次源码的 Mac 编译与真机验收。

## 日常怎么用

| 用法 | Windows 默认操作 | 结果 |
|---|---|---|
| 普通语音 | 点入文本框，按住左 Alt 说话，松开 | 将识别文字填回原输入框 |
| 说中文、练英语 | 开启跟读，按住右 Ctrl + 右 Alt 说中文，松开后听示范、读英文 | 读完填入原中文；开启「发英」后填入英文译文 |
| 说英文、练中文 | 关闭「学习英语」方向开关，用跟读键说英文 | 跟读中文后填入原文或所选译文 |
| 划词翻译 | 选中文字，点翻译图标 | 本机中英互译 |

Mac 普通语音默认按住左 Option，跟读键以设置页保存的组合为准；右键「服务 → 用 Vakyu 翻译」提供划词翻译。Windows 和 Mac 都只填入文字，不自动按发送键；`Esc` 取消本轮。

跟读默认关闭。开启后可在浮窗切换跟读与发译文；已通过的词保留进度，可继续补读、重听、查本地词卡，或主动整句重读。它是日常朗读练习工具，不是专业发音评分或考试系统。

## 这次更新

- 跟读、说中发英和划词翻译使用本机引擎，去掉翻译 API 的配置门槛。
- Windows 英语示范使用随包自然人声，女声默认 180、男声 368；切换音色下次示范生效。
- Windows 提前静默预热朗读模型与推理图，切换男女声复用模型，重听上一句复用内存音频，减少等候。长时间空闲后仍会释放模型内存。
- Mac 新功能在独立源码分支合入构建 9 的基础，保留原有填入兼容处理；新 PKG 与 Mac 真机验收仍待完成。

本机固定短句测量：优化前模型加载约 1.48 秒、第一次合成约 2.03 秒；静默预热完成后的首次合成约 0.15 秒，重听无需再次合成。这是本次机器的单次测量，不代表所有硬件、长句或完整语音交互耗时。

## 隐私与离线使用

- 语音识别、本机翻译、词卡及示范朗读不需要将录音或句子发送给项目服务器；自然人声缓存只存在内存中，不保存音频文件。
- 安装包不包含 API 密钥或作者个人配置。个人词库和学习数据留在本机。
- Windows 随包翻译为 CTranslate2 + OPUS-MT，自然人声为 Piper + sherpa-onnx；Mac 翻译使用 macOS 15+ 的 Translation 框架，需先安装中英语言。
- 项目仍保留**可选云联想**，默认关闭。主动开启它时会调用你配置的服务；上述离线语音、翻译和跟读不依赖它。

## 兼容范围与测试状态

Windows 提供 x64 测试安装包，不支持此包运行在 32 位 Windows 或 Windows ARM。Mac 本次交付为 Apple Silicon 源码预览，无法把 Windows 上的共享核心测试视为 Mac 安装、麦克风或跨应用验证。iOS 暂无本仓库输入法安装包；Linux 保留上游实现，新增语音功能未验证。

密码框、系统安全界面、管理员权限窗口和部分自绘输入框可能不支持填入。测试安装包尚无可信发布者签名；Mac 开发包尚未公证。请从本仓库 Release 下载并核对 SHA-256。当前分发验证不等于全新系统上的完整安装、卸载和各应用验收。

## 文档与参与

- [Vakyu 快速上手](docs/user/getting-started/vakyu-quickstart.md)
- [Mac 使用说明](docs/user/getting-started/macos-voice.md)
- [Windows 打包与验证](apps/windows/installer/README.md)
- [开发约定](docs/contributing.md)

反馈时请提供版本、操作系统、目标软件和复现步骤，避免上传私人文本、录音和密钥。

## 关于与许可

Vakyu 希望让语言练习融入日常表达。官网：[vakyu.com](https://vakyu.com)。

本项目基于 [Qingjian / 青简](https://github.com/qingjian-team/qingjian)，感谢 [@yenharvey](https://github.com/yenharvey)、qingjian-team 和所有上游贡献者。Vakyu 是独立衍生项目，署名不表示上游认可或担保本发行版，详见 [UPSTREAM.md](UPSTREAM.md)。

代码采用 [GPL-3.0-or-later](LICENSE)，再分发须保留版权及许可并提供对应源码；第三方词典、运行库与模型遵循各自许可，详见随包声明。内部部分目录、包名和兼容路径沿用 Qingjian。
