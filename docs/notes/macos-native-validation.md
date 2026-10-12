# Apple Silicon 原生验证记录

验证机器：arm64，macOS 26.4.1（25E253），Rust 1.96.0，Apple Swift 6.3。
源码从交接 ZIP 解压，原始 HANDOFF_MANIFEST 的 1328 项 SHA256 全部匹配。
保留现有 IMK、Swift 宿主、共享跟读核心、界面资源及兼容数据路径。

## 已完成修复

- 补齐 Mac 云服务对 `PredictError::IncompleteReply` 的处理，解决原生 Rust 编译失败；删除无用导入。
- 完整语音包在构建前要求产品词库、语言模型和两种含章模型存在，避免退回样例数据。
- 成品检查增加完整产品数据检查，并逐个检查 Python 运行时的 Mach-O 是否包含 arm64。
- 更正 companion 构建说明：当前 Command Line Tools 可以编译宿主，但不提供 XCTest。
- 简化服务提交确认和菜单图标初始化的条件表达式，通过 Rust 1.96 定向 Clippy。
- 修复 CRLF 导致打包版本号夹带回车；版本提取现兼容 Windows 换行。
- 修复 `lipo -verify_arch` 的参数顺序，输入文件必须放在架构列表之前。
- 签名会修改 Mach-O 文件：新增 `runtime_manifest.py`，内层签名后重算运行时清单，外层 app 再签名封存；成品检查同时核对清单。
- 移除运行时不需要的标准库 venv 模块，解决最终分发规则拦截。

## 已执行验证

- Rust 原生测试：Mac 输入法 16 项、Info.plist 契约 2 项、共享跟读 23 项、语音服务 17 项通过。
- 共享跟读另有 1 项需要专用英文 ASR 回放文件，按原声明忽略。
- Rust 两个发布二进制和 Swift 宿主 release 均已原生编译、链接成功，全部为 arm64。
- Swift 宿主 debug 也已编译成功；19 项 XCTest 尚未运行，当前机器缺少完整 Xcode/XCTest。
- 最终源码定向 Clippy：`--offline --locked --no-deps -p qingjian-macos -p qingjian-practice -p vakyu-voice-service --all-targets -- -D warnings` 通过。
- 发布语音服务的进程协议回归通过，验证双入口、禁用跟读、PCM、单次确认提交、设置白名单和退出。
- Mac 外部 PCM 测试 9 项通过，使用最终 ARM64 Python/NumPy 再次确认。
- 共享 Python 识别、热词和音量测试 29 项通过。Windows 麦克风测试不属于 Mac 采音路径，未安装 sounddevice。
- data-v3 完整产品数据按 data.lock 的 SHA256 校验通过。
- Python、Sherpa ONNX、NumPy、中英语音模型均按 sources.json 校验并完成模型加载。
- 真实 ARM64 模型通过 JSON 行协议回放上游 4 个中英混合音频样本及 10 秒静音；均正常结束，静音结果为空，stderr 为空。这不代表人工语音准确率验收通过。
- plist/XML、shell 语法和源码静态打包检查通过。
- 更新后的运行时重新准备和真实模型加载通过；2584 个文件通过运行时哈希和分发排除检查。清单另验证了修改内容和增加意外文件均被拒绝，重新生成后可通过。

原生 Rust 测试日志在 `target/validation/rust-tests.log`；真实模型回放结果在 `target/validation/model-replay.json`。

## 首次打包与安装

2026-10-09，工具审批恢复后完成全部打包步骤。`target/Vakyu.app` 是完整成品，`check_port.py --bundle` 的完整数据、运行时哈希、全部 Mach-O arm64 架构、逐层签名和分发排除检查均通过。

- 安装包：`target/pkg/vakyu-0.1.5-dev-unknown-macos-arm64.pkg`，约 324 MiB。
- SHA256：`73530fa77aaee27ea13529523162c521eedc24ed4963125dd07cc835fc0c5322`。
- 已安装到：`~/Library/Input Methods/Vakyu.app`。
- 使用打包脚本相同的 ditto 参数复制已验证成品，安装后文件与构建 app 完全一致，`codesign --verify --deep --strict` 通过。
- 已从安装位置执行 `--register`：输入源注册并启用成功；自动选中返回 macOS 错误 -50，需用户从输入法菜单手动选择 Vakyu。
- 已执行随包 `install_voice_agent.sh`，`gui/501/app.vakyu.voice` 状态为 running，首次启动无退出；宿主、Rust 服务和 Python 识别器是唯一的一组父子进程。
- 已安装语音服务的进程协议回归通过；已安装、签名后的 Python 从安装位置加载真实模型返回 `ready`。
- 用户目录仅安装一份 Vakyu，未创建系统级副本；现有 WeType 未修改。

App 为本地开发签名，pkg 未签名、未公证。打包日志在 `target/validation/package-complete.log`，输入源注册结果在 `target/validation/input-source-install.log`。

此前的 lipo 参数、运行时清单和 venv 分发问题已修复并通过实际成品检查。工具审批曾返回 404/429，此次批准后完成首次安装。随后用户实际操作暴露了下述问题，首次安装不能视为功能验收。

## 2026-10-09 用户实测后的修复与更新安装

用户报告右 Command＋右 Option 无法录入、麦克风授权后崩溃、输入源不能打字以及菜单图标不居中。

- 快捷键录入原先只识别右 Control＋右 Option 和单 Option。现在新增右 Command＋右 Option，规范配置值 `right_win+right_alt`，Swift、共享 Rust 校验与服务保存一致；纯修饰组合全松开才确认，继续追加字母有效，非法组合不退化成单 Option。默认键与用户既有配置不变。
- 22:10 的两份 VakyuVoice 崩溃报告均为 TCC 隐私终止。tccd 将麦克风请求归属于外层 `app.qingjian.inputmethod`，而用途说明仅存在于嵌套语音 app。外层现已补齐，源码和成品校验同时要求两层声明；新增检查能拒绝旧成品。
- imklaunchagent 明确拒绝旧 `app.qingjian.inputmethod_Connection`，请求无 endpoint，IMK 未启动。系统诊断得到期望连接名 `app.qingjian.inputmethod.Connection`，plist、fallback 和测试已统一修正；NSApplication 也提前到创建候选窗之前初始化。
- 原菜单 PNG 本身几何居中，但 NSImage 逻辑尺寸为 32pt，两个状态项又分别设为 20/22pt。现在菜单专用资源统一 18pt（36px、144dpi），主体最长边 16pt、四边 2px 留白；两种状态项明确布局和等比缩小。原小鸡图及完整字标不变，系统菜单最终视觉仍需用户确认。

验证结果：Mac 16 项单元测试和 2 项 plist 契约、共享快捷键 5 项、语音服务 17 项均通过；定向 Clippy 通过。Swift release 原生编译通过，纯 Swift 键盘回放 140 条断言通过。当前 29 项 XCTest 未执行，原因仍为缺少完整 Xcode。回放代码、命令及输出保存在 `target/validation/keyboard-replay.*`。

新版完整包重新通过运行时哈希、全部 Mach-O arm64 架构、签名和资源排除检查，已于 22:27 更新安装至原用户级路径。旧 app 备份在 `target/Vakyu-before-fixes-20261009-222706`，没有改动个人配置、学习资料或 WeType。新包路径仍为 `target/pkg/vakyu-0.1.5-dev-unknown-macos-arm64.pkg`，SHA256 为 `afda105126e3a3497273ce35b4db0401977bce2b06a22410c59eb1c8f5d5982a`。

更新后的 `--register` 输出“输入源已注册、启用并切成当前输入源”，不再返回 -50。IMK 已从安装路径启动，日志确认词库和本地语言模型加载；语音 LaunchAgent `runs=1`、从未退出，宿主/服务/Python 进程稳定。已安装服务的隔离协议回归与严格签名校验通过。

界面工具间歇报 ScreenCaptureKit -3811、连接关闭和无窗口应用超时。首次新建 TextEdit 文稿逐键 `nihao` 后出现“你好”，但随后只读 TIS 与会话日志表明该次实际仍为 WeType，不能算 Vakyu 验收。22:38 在 TextEdit 前台经系统快捷键明确切换，探针确认 `app.qingjian.inputmethod.Hans` selected=1；逐键 `zhongwen` 仍为英文，Vakyu 没有收到新的会话请求。

已查到缓存证据：TextEdit PID 85153 在 22:25:18（升级前）创建输入会话并被旧连接名拒绝，22:30:11 新版 endpoint 已成功注册到 imklaunchagent，但这个旧 TextEdit 进程未重新请求。下一步为只重开测试 TextEdit 并重新验证。通过 UI 退出时进入测试文稿保存对话框，正准备保存至 workspace 的验证目录，Mac 锁屏导致工具无法继续。需要用户解锁后接续，不能把当前结果宣称为中文输入已修复验收。仍未代用户申请麦克风权限或真实录音，右侧实体组合键与菜单视觉也未作最终验收。

本轮日志：`target/validation/macos-fix-tests.log`、`keyboard-config-tests.log`、`service-fix-tests.log`、`fix-clippy.log`、`package-fixes.log`、`input-source-fixes-install.log`。

## 2026-10-10 新 TextEdit 复现与模式声明排查

继续验证时使用新 TextEdit 进程，自动化界面中选择 Vakyu 的 `app.qingjian.inputmethod.Hans` 模式后，逐键测试仍显示英文，Vakyu 没有收到新的输入会话。但随后在 00:10、00:11 连续执行的只读 `NSWorkspace` 前台探针均返回 `app.vakyu.voice`（PID 87284），而不是正在测试的 TextEdit；对 TextEdit 执行 Raise 并点击窗口坐标后，前台仍未改变。

因此，这次新 TextEdit 的英文结果不能作为可靠的前台输入验收，不能排除自动化向后台窗口投递而未经过 IMK。旧 TextEdit 会话缓存尚不能作为完整解释，但此次测试也不足以将其排除或确认另一根因。下一次验收必须先确认 TextEdit 是系统前台应用，再核对输入源、会话与中文上屏；目前仍未通过 Vakyu 中文输入验收。

只读 TIS 检查发现，旧包的 Hans 模式被标为 ASCII-capable。它声明 `tsInputModeScriptKey=smUnicodeScript`，模式 repertoire 同时含 `Hans`、`Latn`；本机系统拼音与 WeType 的中文模式使用 `smSimpChinese`，模式 repertoire 为 `Hans`。该差异提供了待验证的假设，尚不能认定为会话未创建的根因。

当前受控修正 `Info.plist`：Hans 模式脚本改为 `smSimpChinese`，模式 repertoire 仅保留 `Hans`；父输入法 repertoire 仍保留 `Hans`、`Latn`。模式契约测试 2 项通过，完整 Rust release 重新编译通过。`iconutil` 在沙箱内失败后，按正常权限流程重试并完成打包。

2026-10-10 00:19:53，新包已更新安装到原 `~/Library/Input Methods/Vakyu.app`，`codesign --verify --deep --strict` 通过。更新前备份在 `target/Vakyu-before-input-mode-20261010`。已安装 `Info.plist` 与 `target/Vakyu.app` 一致，主二进制 SHA256 一致，均为 `a54dc08a4513dcba9e58bc6c015b3542834ab3bd7419e134a98cf84e4d244dff`。安装包仍为 `target/pkg/vakyu-0.1.5-dev-unknown-macos-arm64.pkg`，本次 SHA256 为 `7b50f9a32106bfac809289be019b0e87da7217821d9873671938e44862c56bb2`。构建与测试日志为 `target/validation/package-input-mode.log`、`target/validation/input-mode-tests.log`。

安装后只读 TIS 检查确认 Hans 模式的 `IsASCIICapable` 从 1 变为 0，`enabled=1`、`selected=1`；当时系统前台为 Codex。该结果仅证明模式属性的 A/B 变化成立，不能证明菜单不再跳回、IMK 会话已建立或中文候选已正常显示。已异步请求用户在前台空白输入框物理输入 `nihao` 并按空格、不发送，尚未收到测试结果。中文输入仍待这项实际验收，模式声明调整尚不能认定为已确认的输入故障根因。

更新时发现旧语音独立实例阻止新版 LaunchAgent 启动，已关闭旧实例并通过 kickstart 启动新版。此操作不等于语音功能验收；真实录音、权限申请及右侧组合键仍需实际测试。

00:25 从安装路径重新启动 IMK 后，日志确认控制器、产品词库、候选渲染器和本地模型均完成加载。直接从开发终端启动时要移除继承的空 `RUST_LOG`，否则过滤器只留下错误，看不到 info 级启动和会话记录；未开启逐键 debug。启动线程采样显示进程已进入正常 AppKit 事件循环，记录在 `target/validation/input-mode-startup-sample.txt`。输入源读回与服务进程快照保存在 `target/validation/input-mode-installed.log`。

普通键盘中文输入不依赖语音的麦克风、辅助功能或输入监控权限。当前选中输入源却只有英文、未建立 Vakyu 会话的问题，应继续检查输入法接入；反复申请语音权限不能作为它的修复步骤。

## 2026-10-10 语音快捷键权限诊断与恢复更新

用户报告普通语音键和跟读键均不工作。00:21:57 的 tccd 日志明确显示，原授权的 code requirement 与当前安装的 ad-hoc 签名 cdhash 不匹配，辅助功能与输入监控访问均被拒绝；归属于外层 app 的麦克风授权也存在同样的身份不匹配。相同安装路径与同名权限项目不足以证明当前版本已获授权。

本次修正监听器健康检查与恢复：核对事件端口有效性及启用状态，失效后重建；权限状态每 5 秒只读刷新，手动“刷新权限”立即检查；“重新启动语音服务”同时重建按键监听器。自动刷新不申请系统权限。原界面保留，普通语音键仍为 `left_alt`。用户明确同意将跟读键由 `win+v` 改为 `right_win+right_alt`，原配置备份为配置目录内的 `config.toml.before-right-shortcut-20261010`。

验证与更新结果：

- Swift release 编译通过，耗时 6.79 秒；监听器模拟回放 32 条断言通过。新增 7 项 XCTest 语法检查通过，但缺少完整 Xcode，尚未执行 XCTest。
- 完整 pkg 的资源、运行时清单和签名检查通过。`target/pkg/vakyu-0.1.5-dev-unknown-macos-arm64.pkg` 该次 SHA256 为 `6a27d248c8eb20abfa292b25e459e474597bd83e04654333a987ad6af149e770`。
- 新版于 00:38 安装到原 `~/Library/Input Methods/Vakyu.app`；更新前 app 备份在 `target/Vakyu-before-voice-permissions-20261010`。安装后的语音宿主二进制与 `target` 成品经 `cmp` 确认一致，安装后 `codesign --verify --deep --strict` 通过。
- 真实语音宿主 PID 7946 的 00:38:28.910 启动日志为 `mic=false ax=false input=false tap=false ready=true enabled=true practice=true`：模型及功能开关就绪，但系统权限和按键监听尚未就绪。

凌晨记录时，系统设置仍显示“允许 Vakyu 启用 Vakyu？”模态，用户当时尚未点击允许。该阶段未代用户操作系统授权，也未执行真实录音；语音验收阻塞于 macOS 授权，不能把构建、安装或模拟回放通过写成两种语音快捷键已可用。该模态在上午检查时已消失，后续状态见下文。为避免再次改变签名身份，此版本在用户完成授权测试前不再重新编译或重签语音宿主。

本轮日志：`target/validation/voice-permission-build.log`、`target/validation/monitor-replay.log`、`target/validation/package-voice-permissions.log`、`target/validation/voice-permission-installed.log`。

## 2026-10-10 菜单图标偏低的照片复核

用户新照片仍显示黄色小鸡低于左右菜单栏图标，说明之前将透明边界及资源尺寸统一为 18pt 不足以解决用户所见问题。照片中没有中 / 英文字，目前尚不能确认它属于语音状态项还是系统输入源；不能把它直接归因于其中一条绘制路径。

对已安装 PNG 与打包产物逐字比较一致，SHA256 均为 `3271734b0704547ddef5ee916ce74430298522b36e750f5e1ad1b502cf7d3c92`。`NSImage` 与 `NSBundle.image(forResource:)` 都识别为 18×18pt，alignmentRect 为完整矩形。按现有配置离线构造真实 `NSStatusBarButtonCell`，在 22、24、37pt 高度下 imageRect 的中心均与控件中心一致；离屏渲染与系统 Wi-Fi 符号也处于同一中心。这验证了常规状态项布局，并不等于验证实际菜单栏上下文，因此未给 Swift 或 Rust 状态项盲加偏移。

本机 WeType 的顶层与三个模式图标键均使用 `menu_icon.pdf`；其 PDF MediaBox、CropBox、`NSImage.size`、alignmentRect 都是 16×16pt，没有额外基线偏移。当前受控候选修正为：仅系统输入源换成新名称 `vakyu-input-source.pdf`，使用 16×16pt 彩色 PDF，并保持原小鸡宽高比与透明角；两个自有状态项继续使用原 18pt PNG，生成后 SHA256 与已安装版本完全相同。应用图、完整字标及语音宿主代码与资源不因该项修改而改变。

离线 PDF 校验通过：单页、MediaBox 与 CropBox 均为 16×16pt、旋转角 0；渲染为 256×256px 后透明边界为 `(0,2)-(256,253)`，中心误差不超过 0.5px（0.03125pt），透明角保留。两个 plist 契约测试、源码打包检查与 shell/plist 语法检查通过。生成器及资源校验已更新；该候选尚未通过实际菜单视觉验收，不能宣称照片中的偏低已修复。

离屏布局与 PDF 检查脚本、预览保存在 `target/validation/status-icon-layout.*`、`input-source-icon-check.*`、`vakyu-input-source.pdf`、`vakyu-input-source-preview.png`；PNG 不变对照为 `target/validation/vakyu-menu-unchanged.png`。

图标更新已安装至原用户级路径并成功执行 `--register`，更新前 app 备份在 `target/Vakyu-before-system-icon-20261010`。成品、安装位置及包内 app 严格签名校验通过；安装包 SHA256 更新为 `36ba32f0df48ed40896da44bd23789d1af305e7d60955c96b8f6b519880faf19`。本次仅更新外层图标资源与声明，语音宿主全树 2593 项哈希保持相同，记录在 `target/validation/icon-helper-preservation.json`；宿主 CDHash 仍为 `925067e427416056222913bd29bc6164254cdf32`。

安装后的只读 TIS 查询确认，父输入源与 `app.qingjian.inputmethod.Hans` 的 `IconImageURL` 均已指向安装位置的 `Contents/Resources/vakyu-input-source.pdf`，记录在 `target/validation/input-source-icon-installed.log`。查询未切换输入源、启动或移除应用。新 PDF 已被系统登记采用，但照片对应图标的来源及实际垂直位置仍待用户确认，视觉验收尚未完成。

## 2026-10-10 上午权限状态确认

上午系统设置中的旧启用模态已消失。系统设置曾显示辅助功能与输入监控的同名项目已打开，而宿主实际检查仍为未授权，符合旧签名身份失配的诊断。期间一次系统设置列表行选择被工具自动审批拒绝，没有通过该操作授予或修改任何系统权限。11:41 用户确认已重新添加辅助功能与输入监控权限。

真实语音宿主 PID 20137 在 11:49:29 的状态为 `mic=false ax=true input=true tap=true ready=true enabled=true practice=true`。辅助功能、输入监控与按键监听现已恢复，模型与两个功能开关就绪；尚缺麦克风授权。已请用户点击麦克风允许，未代用户授权、未执行真实录音。已稳定的语音宿主不再为本轮图标修改而重新编译或重签。

用户随后点击允许麦克风。11:52:09.780，同一宿主 PID 20137 的实际状态更新为 `mic=true ax=true input=true tap=true ready=true enabled=true practice=true`：三项权限、按键监听及模型均已就绪。已异步请用户在空白文本框按住左 Option 说“你好这是语音测试”，松开后只检查填入、不发送；尚未收到真实录音测试结果，因此不能把就绪状态等同于录音与填入验收通过。

只读凭据检查仅输出布尔结果 `translation_credential_configured=false`：`config.predict.api_key` 及所配置 `api_key_env` 对应的 `.env` 值均未配置，没有输出密钥内容或发送网络请求。跟读开始时的 `VoicePracticeTranslator` 需要该配置，因此麦克风授权后，真实跟读验收仍须由用户先在“云服务”设置中配置翻译凭据；普通本地语音识别不因此被阻塞。

## 2026-10-10 松键不停录与连接测试反馈

用户确认已松开左 Option，浮窗仍显示录音，同时没有填入文本框；这不是按住说话的正常停顿行为。此前三项权限已实际生效，本次不再归因于权限不足。11:57 至 11:58 主输入法日志有八次“云服务连通性测试成功”，证明用户新填的 API 配置已经可用；此前缺少凭据的记录仅代表旧状态。本轮诊断未重新发送云请求或输出凭据、录音及正文。

12:01–12:02 的短时只读线程采样显示 Swift 主线程在正常事件循环等待，Rust 服务等待命令、写队列无积压，Python 控制输入与识别工作线程也在等待。没有观察到麦克风收尾、AX 或模型推理阻塞。该时点快照不能证明所有时刻都没有延迟，但不支持持续录音是识别器计算积压导致的结论。

已离线构造 KeyboardMonitor 的确定回归：左 Option 的 flagsChanged 松开事件 flags=0，但全局会话 keyState 仍返回按下。旧代码完全用全局状态覆盖事件，导致 heldMode 保持 dictation，release 回调为零；新实现以事件修饰位和左右位为准。这个回放证明旧实现的缺陷，不等于已经捕获用户真实键盘在本机发生了相同快照差异。

协调器松键后立即显示“正在处理…”，关闭录音提示；队列里迟到的初始 dictation 聆听事件不能重新显示录音。已采到的音频仍先排空，再把 release 送给共享服务；自动跟读录音仍由 practice 事件控制。AX 通信超时移到首次属性访问之前，作为输入目标捕获的延迟加固，不把它写成此次持续录音的已证实根因。新增诊断仅记录松键、音频排空毫秒和填入成功布尔值。

“测试连接”现在在按钮旁持续显示测试中、成功或失败，测试时禁用重复点击，完成与超时后恢复；普通配置刷新不清空局部结果。保留窗口底部原提示，没有改用户 API 配置。

验证与安装结果：

- Swift release 完整编译通过。新增松键回放 402 条、既有监听恢复 32 条及快捷键 140 条断言全部通过，共 574 条。另以实际协调器源码和内存替身回放松键、迟到聆听、排空顺序、自动跟读、短按及取消后的旧回调，全部通过。完整 XCTest 仍未执行。
- Rust Mac 16 单元测试与 2 plist 契约通过，定向 `cargo check`、本包 `clippy --no-deps -D warnings` 和修改文件格式检查通过。全依赖 Clippy 遇到既有 `qingjian-learning/.../similarity.rs:14` 的 `needless_borrow`，本轮未扩大修改范围。
- 构建成品的真实模型加载、运行时哈希、ARM64 架构、资源排除和逐层签名检查通过；成品与安装位置的语音服务隔离协议回归通过，不访问真实麦克风或外网。
- 新包于 12:11:57 更新安装到原用户级路径。主 IMK 与语音宿主二进制逐字比较一致，配置和凭据文件更新前后哈希相同；API 与已保存的两个入口均保留。
- 旧应用备份：`target/Vakyu-before-voice-release-20261010-121145`。包仍为 `target/pkg/vakyu-0.1.5-dev-unknown-macos-arm64.pkg`，SHA256 `c5bbfcdefe932c189fa4e5459d19e9acd2e820011f7692e35049c53b74c77e58`。
- 新语音宿主 CDHash 为 `9d964b70710407baaac31ab30fea65dfafcbe17c`，LaunchAgent PID 30574、服务 30594、Python 30595，启动运行且未退出。12:12:00.353 的实际状态为 `mic=false ax=false input=false tap=false ready=true enabled=true practice=true`，确认开发签名更新后旧权限没有继承。已请用户重新添加当前版本授权，尚未进行真实录音复测。
- 界面工具对 bundle ID 因构建与安装副本返回歧义，改用已安装绝对路径后读取超时；未据此声称云连接局部状态已完成视觉验收，也未以其他自动化绕过。真实端到端填入和感知延迟仍待授权后的用户录音结果。

线程证据为 `target/validation/{voice,service,python}-release-hang-stack.txt`；键盘验证命令和修前/修后证据见 `keyboard-release-validation.md`，协调器回放在 `coordinator-release-review/`。本轮打包与安装日志为 `package-voice-release.log`、`voice-release-installed.log`，均在 `target/validation/`。

## 2026-10-10 重新添加后仍未授权：旧备份被系统选中

用户重新添加并刷新后仍不可用。12:17:51 的 tccd 记录明确显示，系统设置提交的 `TCCAccessSetInternal` 指向 `target/Vakyu-before-system-icon-20261010/Contents/Resources/VakyuVoice.app`，记录的 code requirement 仍为旧 CDHash `925067e427416056222913bd29bc6164254cdf32`；运行实例 PID 30845 的当前代码为 `9d964b70710407baaac31ab30fea65dfafcbe17c`，因此 Accessibility 和 ListenEvent 都继续被拒绝。此时单纯刷新或重启不能将旧授权转换为新授权。

应用登记库里实际存在五个旧备份的嵌套 helper。备份外层目录虽未以 `.app` 结尾，内部 `VakyuVoice.app` 仍可被系统发现。已精确注销自有备份及构建副本的登记；五个备份逐文件 SHA256、符号链接核验后归档到 `target/app-backups/<原备份目录名>.tar`，移除可重新发现的解包副本。备份可从 tar 恢复；前文提到的五个历史备份路径现在均对应此归档目录中的同名 tar。

随后重新登记安装位置的外层 app 与 helper，登记库只剩 `~/Library/Input Methods/Vakyu.app` 和其内 `Contents/Resources/VakyuVoice.app`。没有修改 TCC 数据库、权限、用户配置或程序签名。CDHash 保持 `9d964b70710407baaac31ab30fea65dfafcbe17c`。

系统先前“退出并重新打开”产生了独立 helper，LaunchAgent 正常退出后未再拉起；旧 `voice-helper.log` 不能代表该独立实例。已停止准确安装路径的 PID 30845，再由 LaunchAgent 启动 PID 31249。12:23:10.096 实际状态为 `mic=true ax=false input=false tap=false ready=true enabled=true practice=true`。麦克风已恢复，仅辅助功能与输入监控还需要用户按准确安装路径重新添加，已给出 ⌘⇧G 定位路径，等待完成。

证据与整理脚本为 `target/validation/isolate-backup-apps.{py,log}`、`registered-vakyu-after-cleanup.log`。此步骤仅整理应用登记和已核验回滚归档，不需要重新构建或重签。

## 2026-10-10 代为完成准确路径授权并核验

用户不希望继续手动处理权限，改由界面工具接续。开始时系统设置的“打开”面板把完整路径放在了搜索框，未实际定位到安装目录；通过真正的 ⌘⇧G “前往文件夹”进入已安装 helper 的绝对路径，核对选中 URL 后添加辅助功能。12:27:49.551，实际宿主已读到 `ax=true`。

输入监控列表仍有旧记录。界面工具点击 SwiftUI row 不能可靠选中，使用列表方向键并核对 `VakyuVoice.app` selected 后才删除；其他应用项目未修改。随后通过 ⌘⇧G 按准确安装路径重新添加，并在系统弹窗点击“退出并重新打开”。没有写 TCC 数据库、重签或更换程序。

最后把系统重开的独立实例交回既有 LaunchAgent 管理，以程序实际状态核验。12:33:11.821，宿主 PID 31506 的日志为 `mic=true ax=true input=true tap=true ready=true enabled=true practice=true`。三项权限、事件监听与模型均已恢复，证据为 `target/validation/voice-permissions-final.log`。权限问题已处理完成，不再要求用户重复添加；已请求用户真实录音确认松键结束、原框填入和延迟，尚未收到结果。

## 2026-10-10 权限就绪但按键无反应：状态机修复

用户明确在 Codex 中测试。12:33 的宿主权限与监听均已生效，不能继续把“没有反应”归因于缺权限。12:42 的短线程采样主线程处于正常事件循环等待；尚未捕获用户真实按键的拒绝原因。电脑控制工具明确禁止操作 Codex，因此未通过其他工具检查其输入框或自动化测试；后续 Codex 的真实录音由用户亲自验证。

离线回放复现三条独立缺陷：启动采样残留普通键且丢失 keyUp 时阻塞永久不清除；Fn63 的 flagsChanged 松开未更新集合；关闭未在录入的设置窗口仍 endRecording/invalidate，使第一次按下被忽略。修复仅调整原生监听状态机，保留当前修饰键事件快照优先、重连及残余键全部松开保护。事件前清理旧普通键会先补发旧轮次 release，再允许新按下；不会把方向键/F 键的 secondaryFn 当作物理 Fn。

新增快捷键回放 74 项与既有 574 项，共 648 条断言通过，原失败基线保留。完整命令与证据为 `target/validation/keyboard-recovery-validation.md`。协调器以真实源码和内存替身重跑旧松键/音频排空/自动跟读/取消回归，并新增目标捕获失败提示、即时重试及六种入口阻塞验证；全部通过，见 `coordinator-trigger-review/`。

为避免再次只能看见“没有反应”，新增入口匹配、固定拒绝原因码、目标捕获耗时、长按前释放及实际轮次开始日志；不记录应用正文、按键序列、音频或凭据。尚未开录音的错误使用原非激活紫色浮窗显示五秒，不设置 model.visible、不接收键盘焦点或鼠标、不妨碍直接重试。提示计时器代次防止旧回调关闭新轮次；实际方法体配合替身回归已通过，见 `notice-review/`。这不是 Codex 真实录音验收。

本轮完整构建与 `check_port.py --bundle` 资源、模型、架构、签名核验通过；Swift release 编译 6.89 秒。新包 SHA256 为 `56d4b705261e6783aaa8a641e4511e972ae305ba7edf3c95be6937619ead18d2`，路径仍为 `target/pkg/vakyu-0.1.5-dev-unknown-macos-arm64.pkg`。12:50 已更新原安装位置，主程序与 helper 逐字核对一致，配置和 `.env` 哈希未变。旧版经 2625 个文件校验后归档为 `target/app-backups/Vakyu-before-voice-trigger-20261010-125039.tar`，所有构建/备份应用登记已注销，安装位置重新登记，避免系统权限选错同名副本。

新版 helper CDHash 为 `1d2d036338e51daf6dbf2346e767817ba6585b97`，LaunchAgent PID 41385 于 12:50:53.958 的真实状态为 `mic=false ax=false input=false tap=false ready=true enabled=true practice=true`。开发签名变化后需恢复当前版本的既有授权，不能宣称现在两种语音已可用。界面工具提示 Mac 已锁屏且不能自动解锁；已请用户解锁，其他构建、安装与配置核验已完成。证据为 `package-voice-trigger.log`、`voice-trigger-installed.log`、`isolate-trigger-backup.log`。

用户解锁后继续，通过系统设置重加准确安装路径的辅助功能与输入监控，首次移除旧记录时系统要求触控 ID，用户已亲自完成验证。输入监控授权后已点击系统“退出并重新打开”，随后将独立实例交回原 LaunchAgent。14:06:10.435，真实新版 PID 42866 已确认 `ax=true input=true tap=true ready=true enabled=true practice=true`，但 `mic=false`。

系统麦克风列表的 Vakyu 开关关闭再开启仍不足以恢复访问。限定为 Vakyu 的 tccd 记录明确报外层 `app.qingjian.inputmethod` 的麦克风 code requirement 不匹配，见 `target/validation/voice-trigger-tcc-check.log`。界面工具对准确安装路径与 bundle ID 均报 native pipe closed，无法读取/点击 Vakyu 自有设置窗口；未使用其他 UI 自动化绕过。已请用户仅从应用内点击麦克风“允许”，等待重新申请的实际结果；无需再修改辅助功能与输入监控。新版安装文件与构建成品共 2593 项一致，两个快捷键保留为左 Option 与右 Command＋右 Option。

14:09:12.096，PID 42866 日志已更新为 `mic=true ax=true input=true tap=true ready=true enabled=true practice=true`。用户随后确认已点击允许；该次更新版本的三项授权与监听全部恢复，证据为 `voice-trigger-permissions-resumed.log`。此为随后紧凑浮窗与回填修复安装前的历史状态；最新版本权限见下节。

## 2026-10-10 紧凑浮窗、精确回填与更新安装

用户继续反馈窗口偏大、按 Command 时另一个语音浮窗同时出现、只能语音而不能正常打字，以及语音内容没有填入 Codex。14:12 的宿主日志曾出现三次 `commit ok=true`，但用户明确没有看到填入；不能把日志中的旧成功标记视为目标应用已经接收文字。电脑控制工具禁止访问 Codex，因此没有读取或自动化操作其 UI，也没有换工具绕过限制。

本轮界面修改涉及 7 个文件，保留品牌、单词交互与原操作按钮，首帧区分普通语音和两种跟读方向。按目标将宽、高各缩小约一半：普通浮窗约 360×143，跟读浮窗约 360×255；长词可换行、长内容可滚动。12 种离屏状态通过布局验证，7 张截图经独立视觉复核，未发现遮挡或水平越界。离屏检查不等于新版实际录音、滚动、点击和跟读交互已经验收。

回填改为完整预期文本的精确读回确认，不再因 AX setter 返回 `.success` 就报告成功，也删除粘贴后的宽松 `contains` 判断。有限遍历输入框祖先识别 AXWebArea，网页目标优先受控粘贴；原生 AX 只尝试一个写入路径，连续两次读回确认原值和选区完全未变，并再次核验原焦点、窗口及草稿，才允许一次粘贴回退。部分变化、读不回或无法验证时禁止本轮重复注入，保留剪贴板变更保护，并补齐重入、迟到回调及 UTF-16 选区边界保护。新增日志只含固定路径和确认布尔值，不记录正文或凭据。

Delivery 独立编译无警告，24 个测试方法、126 项断言通过，最终只读复核通过；使用真实投递状态机和内存 AX/粘贴替身，没有向真实应用发送输入。命令、回放和限制见 `target/validation/delivery-verification-validation.md`、`delivery-verification-replay.log`。这项验证不能代替 Codex 实际回填验收。

针对普通键、单 Command 和粘贴修饰键残留另做 753 项定向回放，全部通过，没有新增 Keyboard 生产代码改动，也未重复原有 648 项。当前左 Option／右 Command＋右 Option 配置下，普通字母及 Cmd+A/C/V/Q 直通，纯左右 Command 不触发我方语音，自有粘贴标记不污染键状态；按语音时敲普通字母会中断语音并放行该键。记录在 `target/validation/keyboard-pass-through-diagnosis.md`、`keyboard-pass-through-replay.log`。

本次更新安装前，手动启动当时安装的主 IMK 后，用户用实体键输入 `nihao` 并按空格，确认得到“你好”。这是一项有效的正常中文输入实测，修正了前文尚无可靠实体输入通过的状态；但它不代表本次新版在所有应用中的持续输入已经验收。安装流程因此补齐了主 IMK 的启动，不能只启动语音 helper。

语音冲突排查中已关闭 WeType 的两个语音快捷键及 Siri。用户选择系统听写保持关闭，系统设置已核验为关闭；但用户报告黑色“×／波形／✓”浮窗在听写关闭后仍在 TextEdit 出现，来源仍未确认。用户另确认 Codex 设置中“没有这个绑定”，指单独 Command。右 Command 作为组合前缀会被放行，纯修饰键也无法用 Carbon 独占检测，因此不能仅凭我方快捷键回放通过排除另一个语音入口；也不能把该黑色浮窗归因于 Codex。

构建、安装与保留结果：

- 新包仍为 `target/pkg/vakyu-0.1.5-dev-unknown-macos-arm64.pkg`，SHA256 为 `4a4549e6ff60c937a8ec598ed8a56a826d0406403b2ee3eaec870cdb91aa62b9`；helper CDHash 为 `a3b6eac61a4888cefbc27d0bdb3050dde8967080`。新包严格逐层签名核验通过，运行资源齐全。
- 14:49 已安装到原 `~/Library/Input Methods/Vakyu.app`，`config.toml` 与 `.env` 更新前后哈希一致；已配置的 API、左 Option 与右 Command＋右 Option 均保留，早先缺凭据不是当前阻塞原因。
- 旧版经 2625 个文件核验后归档为 `target/app-backups/Vakyu-before-voice-compact-20261010-144937.tar`，避免旧嵌套 helper 再被系统选作授权对象。
- 本地 `target/validation/install-voice-compact.sh` 已补 EXIT 失败恢复旧包与旧服务，并确保从安装位置执行 IMK 注册、启动及 helper LaunchAgent 安装；这是安装流程修正，没有改动或重签已核验成品。
- 安装后主 IMK PID 54413 与 helper PID 54695 均在运行，分别来自准确安装路径的 `Contents/MacOS/qingjian-macos` 与 `Contents/Resources/VakyuVoice.app/Contents/MacOS/VakyuVoice`。
- 语音服务 smoke 通过，日志为 `target/validation/voice-compact-service-smoke.log`。这不包含真实麦克风、Codex 回填或实际跟读交互验收。

新版安装后已通过系统设置 UI，按准确安装路径重新添加辅助功能与输入监控，用户亲自完成了 Touch ID。14:57:11.700，当前 helper 的实际状态为 `mic=false ax=true input=true tap=true ready=true enabled=true practice=true`。用户随后在 Vakyu 应用内点击麦克风“允许”；14:58:01.822，同一 helper PID 54695 已更新为 `mic=true ax=true input=true tap=true ready=true enabled=true practice=true`，三项权限、按键监听和模型全部就绪，证据为 `target/validation/voice-compact-permissions.log`。前文 12:12 的三项权限失配、14:09 的旧版本全部授权及 14:57 的麦克风未授权均是历史观测，不能代替这一最新状态。

随后用户在 Codex 按住左 Option 说指定测试句并松开，明确回复“已结束并填入文字”。本次安装版本的这一次真实录音、松键结束及 Codex 原框回填已通过用户验收；结果依据用户亲自操作和确认，不是旧成功日志或内存回放。随后用户以右 Command＋右 Option 实际测试跟读，确认“标题、尺寸和跟读都正常”。英语跟读入口、模式区分、缩小后的尺寸与本次跟读流程已通过用户验收；这不代表长口述或全部跨应用场景均已验收。当前宿主日志也显示普通轮次松键后 111 ms 排空音频、359 ms 完成精确回填确认；这是单次观测，不代表所有录音的延迟。实测日志留在 `target/validation/voice-compact-live.log`。

界面进一步检查千问设置，实际“语音输入法”页显示语音输入快捷键为右 Command、语音指令为双击右 Command，总开关已关闭。随后用户明确告知“就是千问导致的，我已经关了”，因此冲突来源由用户确认是千问，且已由用户关闭。助手没有重新开启该开关，也未修改千问其他快捷键。此结论替代本节之前排查阶段的“来源未知”；不再把黑色浮窗归因于 Codex。

## 2026-10-10 微信聊天框误报密码框

用户报告微信普通聊天框提示“密码框不支持语音输入”。旧版 `InputTarget.capture` 将 focused=nil 与实际 secure=true 合并到同一错误，提示本身不足以证明目标是密码框。界面工具初次进入微信设置时出现“为了你的账号安全，请重新登录”，用户重新登录并点文件传输助手空白框后仍复现；因此没有将重新登录当作最终修复。

微信版本为 4.1.13（269602）。其随包主库含 Qt 5.15.14、QAccessibleTextEdit 与“读屏优化模式 / Screen reader optimization mode”。用户授权查看已隐藏私密内容的测试窗口后，工具得到的主窗和设置页仍只有窗口框架及菜单，图像为空白，未猜坐标或通过其他 UI 技术绕过。用户曾回复“开启后已能语音填入”，随后澄清“没找到读屏优化模式”；之前的微信成功结论因此撤回，开关位置与真实填入均需重新验证。没有发送聊天消息，没有修改真实密码控件保护，也没有再次重签或安装。末次采集的 helper 日志仍停留在该设置变更前的拒绝，因此不把旧日志声称为此成功轮次的独立证据。

源码另修复了误导提示及局部焦点兼容：新增 `FocusResolution` 固定前台 PID，只有系统焦点缺失时才查询同一应用的焦点，捕获与验证共用。前台变化、进程不符、安全输入和安全元素仍拒绝；新增固定错误原因与 system/app 来源日志，不含正文。未加入微信白名单、全窗口搜索或盲目粘贴。14 个焦点回归方法、74 条断言通过；既有 Delivery 126 条断言通过，编译无警告，独立只读审核通过。回放保存在 `target/validation/focus-resolution/`。这些源码改动尚未打包安装，微信设置方案目前尚未证实有效；运行版仍是 14:49 安装、14:58 权限恢复的版本，不能声称运行版已包含新的焦点解析器。

## 2026-10-10 微信按 Fn 切换输入法时崩溃

用户提供的报告显示微信 4.1.13（269602）在 macOS 26.4.1 于 15:28:24 崩溃，用户明确触发方式是地球／Fn 键。主线程依次经过 `TSMMessagePortCallBack`、`TUINSCursorUIController.showTextInputMenuHUD:`、`_selectCurrentInputSource`，最后 `CFRelease(NULL)` 触发 EXC_BREAKPOINT/SIGTRAP。该报告证明直接故障位于微信进程内的 Apple 输入法切换浮层；没有 Vakyu 语音、AX 或 IMK 代码栈，不能据此排除输入源登记状态诱发，也不能把责任直接归于某一方。报告未记录当时切换的源/目标 ID 或具体为空的属性。

只读检查安装 plist，其模式、语言、连接与图标声明内部一致；未发现系统级 `/Library/Input Methods/Vakyu.app` 重复安装。磁盘 HIToolbox 偏好中的 SelectedInputSources 为 Vakyu.Hans，但 EnabledInputSources 没有 Vakyu；这可能是缓存写回时间差，不能当作实时状态。为核实准备了 null-safe、无修改的 `target/validation/input-source-crash-audit.swift`，只查询 TIS 当前/启用/选择能力/图标等属性，独立编译通过。运行请求被自动审批服务 503 阻止，程序未执行，没有绕过限制，也没有据此清空偏好、重新注册、切换输入源或宣称修复。

暂时规避方向是避开报告中的 Fn 切换 HUD，使用顶部输入法菜单；菜单路径是否完全无崩溃尚未实测。待实时状态可核验后，针对确定的异常登记做定向修复。此前未安装的焦点提示补丁不覆盖本次系统浮层崩溃，不能拿它当作修复。

## 尚需用户配合与未验收项

仍需完整 Xcode 执行 XCTest。当前锁定的 candle-metal-kernels 0.11 不在构建阶段调用独立 Metal 编译器，不能仅凭 `xcrun metal` 缺失判定 Rust 产品无法构建。

以 14:49 安装的紧凑浮窗／精确回填版本为准：14:58:01.822 已确认麦克风、辅助功能、输入监控、按键监听和模型全部就绪。API 配置已有实际连接成功记录，且本次安装保留了配置与凭据；不再把早先缺凭据或历史权限失配列为当前故障。用户已亲自确认新版左 Option 短句录音松键结束并回填 Codex，以及右 Command＋右 Option 的英语跟读标题、尺寸和流程正常。

英语跟读的一次完整实测已通过；紧凑浮窗的长内容滚动及全部按钮交互仍需扩展验证。Codex 的一次短句实际回填已通过，后续场景仍须由用户亲自验证，不能用旧 `commit ok=true` 或内存回放替代。微信普通聊天框的先前成功记录已由用户后续澄清撤回，当前仍需定位开关并修复、实测。千字长口述、双向跟读和 TextEdit、浏览器、微信、Codex、Muse 的全部原框填入场景仍未完成扩展验收。

普通中文键盘输入已有用户实体 `nihao`＋空格得到“你好”的通过记录；本次新版安装后的持续输入和跨应用表现仍需确认。用户已明确确认黑色“×／波形／✓”语音浮窗来自千问，并已亲自关闭其语音输入功能；系统听写按用户选择保持关闭。系统输入源的新 PDF 图标已经登记采用；本轮最后用户明确回复“已经居中”，顶部输入法图标的实际居中效果已通过目测验收。
