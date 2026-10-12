# Vakyu Apple Silicon 移植记录

目标是对齐现有 Windows 版的功能和流程，最低 macOS 13，Apple 芯片优先。现已在 Apple Silicon 完成 Rust/Swift 原生发布编译、自动测试、完整 pkg 打包及当前用户安装；系统权限和真人应用兼容性仍待验收。实际结果与剩余项目见 [原生验证记录](macos-native-validation.md)。

## 架构与责任

原 Rust IMK 壳保留普通拼音/候选/词库/学习/设置。`VakyuVoice.app` 是独立 Swift 程序，由用户 LaunchAgent 常驻；CGEventTap 收实体按键，AVAudioEngine 采音，AX 锁定原输入目标，AVSpeechSynthesizer 播放本机语音，SwiftUI/AppKit 绘制浮窗。它不重复实现翻译或跟读匹配。

`apps/voice-service` 用 JSON 行协议协调 ASR、异步翻译、完整跟读会话、词卡、经验和交付。协议见 `apps/voice-service/PROTOCOL.md`。新增 `crates/qingjian-practice` 从 Windows 迁出原 Session/Matcher/词卡/View；Windows 也引用同一实现，防止两端识别规则分叉。

`apps/macos/voice-runtime` 接收 base64 Float32LE PCM，不访问麦克风。原生层转换为 16 kHz mono，Python 复用原有流式 Sherpa ONNX Recognition/PracticeContext/BPE/音量模块。模型只加载一次、CPU 两线程，没有固定总录音时限，不存录音。麦克风/后台队列有上限，过载显式失败，不静默丢句后当作成功。

## 已实现的对照范围

| Windows 现有行为 | Mac 实现 |
|---|---|
| 左 Alt 普通语音、右 Ctrl＋右 Alt 口语跟读 | 左 Option 直接口述，右 Control＋右 Option 独立跟读，实体左右区分；跟读键可自定义 |
| 松开结束、实时文字/电平 | 原生采音，异步 PCM；结束先排空已收音频 |
| 长句约千字、停顿继续 | ASR endpoint 分段累计，整轮无固定截止 |
| 英语/中文双向跟读 | 同一 PracticeSession/Matcher，中文拼音与词义 |
| 新手容错/姓名/渐进经验 | 共享核心，两个语言独立经验文件 |
| 重读/跳词/手动放行 | 同一 action 状态机；点击外部不关窗 |
| 点击词卡/IPA/中文拼音/TTS | 离线词典共享，系统声音，不额外查词 token |
| 翻译一次、取消旧请求 | VoicePracticeTranslator；generation/round 隔离 |
| 原话填入，不自动发送 | AX 选区优先，目标进程粘贴回退，核对目标及文本 |
| 紫色/白字/完整字标/托盘 | IMK 设置与候选品牌更新；语音窗下中、字标外露 |
| 关闭云联想仍可语音翻译 | 两者独立；设置接口和密钥控件不随联想关闭而禁用 |

## 配置与数据

复用 `[shortcut]`：`voice_input`、`voice_english_practice`、`voice_practice_learn_english`、`voice_practice_demo`、`voice_practice_shortcut`、`voice_key`、`voice_hold_ms`、`voice_practice_similarity`、`voice_practice_name_similarity`。Mac 新配置默认跟读键 `right_ctrl+right_alt`、普通语音键 `left_alt`；有配置则保留用户选择。普通语音绕开跟读与翻译，跟读开关只控制跟读入口。协议中的 alt 对应 Option、win 对应 Command。

复用 `[predict]` 服务配置，单次跟读最终翻译独立于 `enabled`。密钥来自配置或同目录 `.env` 的 `api_key_env`，只送翻译请求，不进入 IPC、日志或安装包；换密钥会重建翻译器。朗读音色由 Mac 原生设置保存在当前用户的偏好中，选择系统已安装音色。

兼容目录 `~/Library/Application Support/Qingjian/` 与旧输入源 bundle ID 保留。用户可见品牌是 Vakyu；保留旧内部标识是升级兼容所需，不能批量改名导致重复输入源或资料丢失。

## 系统差异与验收边界

- macOS 首次需要麦克风、辅助功能、输入监控授权，不能绕过授权或安全输入。
- Mac 的 Option/Command 与 Windows 的 Alt/Win 键语义不同；纯修饰键不能通过 Carbon 独占注册。系统注册的普通组合键可检测，应用内部快捷键和第三方事件钩子无法完整检测。
- 输入目标的 AX 支持由应用决定。不能验证粘贴成功时提示用户检查，不把发送了事件当作填入成功，不盲目重复粘贴。
- Mac 系统音色、字体栅格化与系统控件无法凭 Windows 测试认定像素/音色完全一致。使用现有图形资产和同色值，需并排截图核对和真机调整。
- 首次输入源注册的系统缓存与全局语音相互独立；全局语音不要求重新打开目标软件。IMK 自身仍遵循 macOS 输入源规则。

## 手动验收清单（待 Mac 完成）

1. Apple 芯片 Mac 构建 pkg，检查所有 Mach-O 含 arm64；深度签名检查通过。干净用户安装，拒绝各权限再逐个开启，界面给出正确提示、模型就绪后能开麦。
2. TextEdit、Safari/Chrome、微信、Codex 和日常应用分别点击输入框，分别用左 Option 普通语音和右 Control＋右 Option 跟读录音。逐项验证只插入原话、不发送；已有应用和新安装应用均无需重启语音组件。
3. 交换两个右侧修饰键按下/松开顺序，左 Control 不能误触跟读；左 Option 单独口述，两入口相互切换及松开残余键不得触发第二轮；Option＋字母普通操作不得误录音。保存 F8、Control＋Option＋E；另一进程占用组合键时保存失败、旧键仍有效，重启后设置保留。
4. 原话录音持续 8–10 分钟或约 1000 字，中间停顿，按键始终按住不得自行结束；检查开头/句尾均完整、CPU/内存不随录音时间持续增长。
5. 学英语开：说中文、读英文、填原中文；关：说英文、读带拼音中文、填原英文。练习总开关关闭时跟读键不启动，左 Option 普通语音始终直接填入。仅跟读翻译产生请求，普通打字关闭联想后不产生云联想请求。
6. 读 “My name is Chen Junqi”，测试近音、大小写标点、闲聊插入、错词重读和停顿；已读前缀保留，新手可跳词/手动放行。中文“银行”“重庆”、数字与拼音姓名按共享规则处理。
7. 点击词语显示释义和 IPA/拼音，读音正确；未知词友好提示。整句示范和单词发音不能自动通过跟读。已读词过渡和真实音量流畅，停止麦克风后电平归零。
8. 点击浮窗内外、切换 Space/全屏/多屏、点击词卡，不自动关闭；Esc 清理且不插字。读完并确认填入才关闭。下轮不残留旧句、进度、词卡或迟到录音结果。
9. 跟读期间改目标应用、关闭目标窗、编辑原框或改选区，禁止误投/覆盖；回到安全原目标后可重试。粘贴失败与确认丢失不得重复插入；剪贴板原内容恢复，期间用户新复制的内容不覆盖。
10. 断网、无密钥、换密钥、模型缺失、拔麦、服务崩溃和重启逐项测试；错误保留可操作状态，旧事件不污染新轮。升级不清词库/经验，卸载停止登录项且默认保留资料。

## 构建检查

入口 `apps/macos/scripts/bundle.sh --pkg`；`prepare_voice_runtime.py` 锁定下载哈希，`bundle_voice.sh` 装配 helper，`sign_bundle.sh` 从内到外签名，`check_port.py --bundle` 校验实际产物。Apple Silicon CI 见 `.github/workflows/macos-port.yml`，需在 GitHub 执行成功才能产生可下载构建。

Windows 已运行的测试与剩余限制记录在本轮 `outputs/vakyu-macos-port-report.md`。不得将未执行的 Mac CI、Swift 测试、权限和应用兼容测试记为通过。

### 2026-10-10 微信兼容性复测状态

已安装焦点缺失提示修正、同应用焦点回退（要求可读原草稿与有效选区）、粘贴50/350毫秒分段精确确认以及系统输入源图标向上1pt校正。Swift release构建、焦点74断言、Delivery179断言、plist2项契约、实际资源与深度签名检查通过。普通语音与英语跟读快捷键、API配置及已有资料保持不变。

微信4.1.13的Fn闪退直接发生于系统输入切换浮窗：按输入源ID查找得到空值后调用CFRelease。新独立进程能查到Vakyu.Hans，仍未确定微信浮窗中的实际失效ID。已刷新系统输入菜单并重新登记安装来源；“菜单选完立即跳回”和Fn崩溃尚需用户在新版上的实际复测，不能因编译通过而认定修复。细节见 `target/validation/wechat-compatibility-20261010.md`。

### 2026-10-12 语音焦点与受限粘贴

修复前分开实测：TextEdit 用户确认开始录音，日志 `focus source=system`、`delivery path=ax_selected_text confirmed=true`；Safari 本地测试框 `focus source=system`、`delivery path=paste confirmed=true`。微信用户确认无法读取输入框，但旧日志未捕获可归属微信的确切原因码，不把 Finder 的 unsupported_editor 误记为微信。运行状态为 mic=true ax=true input=false tap=true ready=true；因此未先撤销已工作的系统授权。

新增默认开启的受限粘贴开关，始终复核原 PID/可见窗口、已知焦点与安全输入；可读取输入框保留原精确交付流程。一次粘贴后不能核对时保留原话与复制入口，不发送、不重试。新增权限可能失效提示与固定签名选择。具体测试、安装及应用复测结果见 `target/validation/voice-focus-20261012-report.md`；未完成的真机项不得标记通过。
