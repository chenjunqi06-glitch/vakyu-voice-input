# Vakyu Windows 离线安装包

安装包面向 Windows 10 22H2（19045）和 Windows 11 **原生 x64** 系统。包含 x64 Server、设置程序、x64 / x86 TSF 输入法组件、自包含 Windows App Runtime、中英流式语音模型、独立 Python、离线词典，以及微软签名的 x64 / x86 Visual C++ 运行库。用户不需要安装 Rust、Python 或开发工具；不支持 32 位 Windows 与 Windows on ARM。

公开包使用 `uiAccess=false`，不安装开发者的自签证书，不修改根证书信任。未签名软件可能显示 Windows 安全提示；向管理员权限窗口输入、系统安全界面以及部分受保护界面的悬浮层存在系统权限限制，不能保证每个控件都接受外部输入。正常语音识别在本机 CPU 上运行，不用显卡。联网翻译需要接收者在设置中配置自己的 API 密钥。

当前只支持一台电脑上的**单个交互登录会话**。IPC 尚未按用户与会话隔离，不承诺快速用户切换后多个账号同时运行、多人远程桌面或终端服务器场景。

## 干净的分发输入

`build.ps1` 默认从仓库旁的工作区目录读取：

```text
outputs/vakyu-distribution-input/
    voice-runtime/       python、app、models、licenses 及来源 / 哈希清单
    data/                dict.qj、lm.qj、glossary-*.qj、english.tsv、dicts、codes
    prerequisites/       vc_redist.x64.exe、vc_redist.x86.exe
    notices/             INSTALL-LICENSE.txt、依赖 / 词典 / 模型的许可证与来源声明
```

这是干净的发布输入，不能指向整个 AppData 或用户资料目录。语音运行时应由固定版本的官方 Python、依赖 wheel、模型和仓库脚本组成。配置、密钥、使用记录、个人词库、录音、日志都不属于发布输入。词典白名单仅复制运行所需格式；不复制开发中间产物或 `._` 资源旁车文件。原项目采用 GPL，转发安装包时应一并提供对应源码及许可证，具体来源见随包声明。

## 构建

需要 MSVC、Rust x64 / i686 目标与 Inno Setup 7.1.0。不需要在构建机安装产品。

离线翻译组件（CTranslate2 + OPUS-MT）第一次需要在能上网的构建机固定版本：用普通 Python 3.12 运行一次 `py -3.12 apps\windows\installer\pin_translation.py --cache ..\outputs\distribution-download-cache`（路径按实际仓库调整，缓存目录必须与下面 `prepare-runtime.ps1` 相同）。它解析 Windows wheel 的 PyPI 地址与 SHA-256、记录 Hugging Face 模型版本，并把官方权重转换成 int8 放进缓存，再写回 `translation-sources.json` 与 `translation-models.json`；提交这两个文件后，以后的构建只校验、不再转换。两个 JSON 还没固定时 `prepare-runtime.ps1` 会直接报错提示。

先从已锁定的上游归档生成干净语音目录。`voice-sources.json` 固定 Python、wheel 和模型归档的版本、下载来源、SHA-256；`voice-models.json` 再固定展开后模型文件的大小和哈希。`prepare-runtime.ps1` 检查两层哈希，生成只读安装可用的 BPE 数据和来源 / 依赖清单；输出目录必须为空，缓存可复用。

```powershell
powershell -ExecutionPolicy Bypass -File apps\windows\installer\prepare-runtime.ps1 `
  -OutputDirectory ..\outputs\vakyu-distribution-input\voice-runtime-clean `
  -CacheDirectory ..\outputs\distribution-download-cache
```

然后准备数据、微软官方签名运行库与第三方声明（见上面的目录布局），构建安装包。以 `voice-runtime-clean` 作为本次干净运行时目录时，需要显式传参；脚本默认目录仍为 `voice-runtime`。

```powershell
$env:QINGJIAN_ISCC = 'C:\Tools\Inno Setup 7\ISCC.exe'
powershell -ExecutionPolicy Bypass -File apps\windows\installer\build.ps1 `
  -VoiceRuntimeDirectory ..\outputs\vakyu-distribution-input\voice-runtime-clean

# 二进制已由本次构建生成时，仍校验架构和 uiAccess，再重新暂存全部文件。
powershell -ExecutionPolicy Bypass -File apps\windows\installer\build.ps1 -SkipBuild `
  -VoiceRuntimeDirectory ..\outputs\vakyu-distribution-input\voice-runtime-clean

# 只准备可检查的安装内容，不编译 setup.exe。
powershell -ExecutionPolicy Bypass -File apps\windows\installer\build.ps1 -SkipBuild -StageOnly `
  -VoiceRuntimeDirectory ..\outputs\vakyu-distribution-input\voice-runtime-clean
```

构建固定使用 `--target x86_64-pc-windows-msvc`，输出来自 `target/x86_64-pc-windows-msvc/release`；x86 TSF 来自 `target/i686-pc-windows-msvc/release`。不会回退拿取陈旧的 `target/release`。默认两条编译任务、低于正常的进程优先级。

本次构建机的 Rust 1.99 没有 i686 标准库，因此 x86 TSF 另用已安装的 Rust 1.96.0 构建，产物在 `target/deploy196-x86/i686-pc-windows-msvc/release`。这种分工具链构建应先分别完成，再传 `-SkipBuild -X86BuildDirectory target\deploy196-x86\i686-pc-windows-msvc\release`；不修改通用默认路径。

可用 `-X64BuildDirectory`、`-X86BuildDirectory`、`-SettingsRuntimeDirectory`、`-VoiceRuntimeDirectory`、`-DataDirectory`、`-VCRedistDirectory`、`-NoticesDirectory` 显式指定输入。`-PackageVersion` 可指定安装包版本；省略时读取 Windows Server 的版本，并为开发版本附加 git 标识。`-Sign` 会直接拒绝：本机自签产物仅能单独用于开发测试，不能混入公开包。

构建前检查包括二进制架构、Server 的 `uiAccess=false`、设置运行时全部文件、必要词典与模型，以及 VC++ 安装器的有效 Microsoft 签名。缺失依赖直接失败。暂存目录每次新建于仓库 `target/installer`；保留旧产物，不执行递归清理。可用 `-StageName distribution-stage-<标识>` 固定本轮目录，已存在时拒绝覆盖；输入与暂存目录不允许链接。`Copy-CleanTree` 排除录音、日志、Python 缓存 / 测试，并拒绝配置和私钥文件。

产物：

```text
target/installer/distribution-stage-<标识>/payload/           最终安装内容
target/installer/distribution-stage-<标识>/prerequisites/     VC++ 安装器
target/installer/vakyu-<版本>-windows-x86_64-setup.exe
target/installer/vakyu-<版本>-windows-x86_64-setup.exe.sha256
```

`payload/distribution-manifest.json` 记录各文件的相对路径、长度与 SHA-256，用于检查安装后是否完整。Inno 只读取该次暂存目录，不能直接捞取整个构建目录或用户文件夹。源码发布归档和首次安装验证由发布流程另行完成。

使用暂存目录里的独立 Python 运行验证，不依赖系统 Python。`verify-distribution.py` 核对完整文件集合、哈希、架构、uiAccess、凭据特征，并在空的用户目录与最小 PATH 下运行模型 Ready → Shutdown 握手。它不开麦、不安装输入法，也不代表全新 Windows 安装 / 卸载或真实应用测试已通过。

```powershell
$releasePython = 'target\installer\distribution-stage-<标识>\payload\voice-runtime\python\python.exe'
& $releasePython -I -B apps\windows\installer\verify-distribution.py `
  --payload target\installer\distribution-stage-<标识>\payload `
  --test-home ..\outputs\distribution-smoke-home `
  --report ..\outputs\distribution-verification.json

cargo metadata --locked --format-version 1 | Set-Content -Encoding UTF8 ..\outputs\vakyu-distribution-cargo-metadata.json
& $releasePython -I -B apps\windows\installer\package-source.py `
  --output ..\outputs\Vakyu-Windows-Distribution\vakyu-source.zip `
  --dependency-index ..\outputs\vakyu-distribution-input\notices\rust-dependencies.json `
  --cargo-cache "$env:USERPROFILE\.cargo\registry\cache" `
  --cargo-metadata ..\outputs\vakyu-distribution-cargo-metadata.json
```

`package-source.py` 归档本次应用工作树及锁定的 Rust normal/build 依赖源码，排除构建目录、密钥和用户数据，并生成源码归档 SHA-256。运行时需要缓存中存在注册表依赖的全部 `.crate` 归档（按 Cargo.lock 校验），Git 依赖从 metadata 指定的 checkout 收入对应版本源码快照；如果缺失，应先获取锁定依赖，不可从源包里直接略去。

## 安装、升级与卸载

安装目录保留 `C:\Program Files\Qingjian` 和旧 AppId，支持已有用户原位升级，界面品牌为 Vakyu。安装器先执行随包的 VC++ 安装器，失败时显示返回码并停止，不覆盖旧程序；成功或已安装更新版本才继续。运行库要求重启时，安装器提示用户，不主动重启。

然后只停止本产品 Server、设置程序和 Server 的语音子进程，复制文件，由 Inno 注册 x64 / x86 文本服务并检查注册错误，为应用容器赋予只读 / 执行权限。不会使用 Restart Manager 批量关闭浏览器、聊天软件或其他宿主。TSF DLL 以版本命名，已打开程序可以继续使用已加载的组件，升级前加载旧组件的程序可能需要重新打开。

许可页面包含项目 GPLv3 和微软运行时的原版完整条款；各自独立适用，不以微软运行时条款限制 GPL 源码权利。`notices/INSTALL-LICENSE.txt` 为必需输入。

安装完成后先以原登录用户调用 Server 的 `--enable-user-profile` 入口，启用该用户的 Vakyu 键盘条目；该入口不录音、不切换默认键盘、不删除其他输入法。然后启动全局语音后台；完成页可勾选打开设置。正常登录首次启动也会幂等补齐当前用户的条目；不以提权管理员身份修改另一用户的键盘列表。

所有用户的启动文件夹各自启动该用户的后台，但当前 IPC 仅支持单个交互登录会话，不应让多个登录账号同时运行。普通语音默认使用左 Alt，口语跟读使用右 Ctrl＋右 Alt；翻译密钥由接收者自行填写，并在设置启用跟读。云联想默认关闭，普通本地语音不需要密钥、不消耗 API token；翻译按 API 服务商规则计费。跟读通过后只填入原文，不自动按发送键。首次使用麦克风仍受 Windows 麦克风隐私设置和设备状态控制。

静默安装支持 Inno 标准 `/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /LOG=<文件>`；静默安装不执行用户键盘初始化、不自动打开程序，待目标用户正常登录启动时处理。交互验证可额外传 `/NOAUTOSTART`，避免新后台接管当前会话，但仍初始化原登录用户的键盘条目。登录时仍按启动快捷方式运行。

升级期间通过 `Global\QingjianInstaller` 阻止新版 DLL 拉起 Server。运行中的旧 Server 先改名再终止，Server 最后落地；同版本重装若取消，尽力把腾位旧文件恢复。旧 DLL 无法立即删除时登记重启后删除。

卸载终止本产品进程，反注册文本服务并删除安装文件、快捷方式。用户的 `%APPDATA%\Qingjian` 配置、个人词库及 `%LOCALAPPDATA%\Qingjian` 历史数据保留；系统共享 VC++ 运行库保留。没有删除用户数据或根证书的操作。

## 自包含设置运行时

`apps/windows/settings/build.rs` 让 `windows-reactor-setup` 将 Windows App Runtime 解到明确的 x64 release 目录。`settings-runtime.txt` 对齐其运行时清单：文件和语言资源必须与设置 exe 同级。升级 `windows-reactor` / `windows-reactor-setup` 时同步核对清单，并在干净系统上验证设置窗口；仅在构建机成功打开不能替代此项验证。
