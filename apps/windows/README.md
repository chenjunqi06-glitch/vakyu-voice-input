# Miracle Windows 输入法

Windows 端是**一个产品、两个产物**，各自一个 package，同放本目录：

| 目录 | package | 产物 | 职责 |
| --- | --- | --- | --- |
| `server/` | `qingjian-windows-server` | `qingjian-server.exe` | 持有唯一的输入内核 `qingjian-core::Engine`，跑在所有应用进程之外 |
| `tsf/` | `qingjian-windows-tsf` | `qingjian_tsf.dll` | TSF 文本服务，被加载进每个应用进程，只做按键转发与文档写入（候选窗口由 Server 自绘） |

```
应用进程 A ── qingjian_tsf.dll ─┐
应用进程 B ── qingjian_tsf.dll ─┼─ 命名管道 \\.\pipe\qingjian ─▶ qingjian-server（唯一的 Engine）
应用进程 C ── qingjian_tsf.dll ─┘
```

## 为什么核心逻辑要在进程外

Windows 的文本服务（TSF，Text Services Framework）是一个 COM DLL（`ITfTextInputProcessor`），
系统会把它加载进**每一个**接受文本输入的应用进程。因此输入内核不能待在 DLL 里（会被复制进几十个进程、
状态无法共享、崩溃会连累宿主应用）。Miracle 照 Weasel（WeaselServer + WeaselTSF）、水杉（Server 进程）的做法：
Engine 只此一份，跑在独立的 Server 进程；每个应用进程里的 TSF DLL 只做两件事——把系统按键翻成协议消息发来、
把 Server 回的候选画到候选窗口。

## 为什么是两个 package 而不是一个

两个产物的依赖集合刻意不同：DLL 只依赖 `qingjian-core`、`qingjian-platform` 与官方 `windows` crate（COM
`implement` 宏），Server 才依赖词库 / 学习 / 翻译 / 云联想（含 tokio）整棵树。合成一个 package 后，DLL 的
编译单元会拉进 Server 的依赖；用 feature 区分也不行，workspace 一起构建时 feature 会统一。crate 边界就是
「DLL 不含 Engine」这条约束的强制手段。判断标准与 macOS 的 IMK 壳一致：换掉平台适配层，不应该需要改
Core 的任何一行。

## 三个部分

- **Server 进程**（`server/`）：装配并持有 Engine（词库 / 语言模型 / 翻译 / 学习），按 `SessionId` 为每个
  应用会话维护各自的组句状态，处理按键、产出候选与上屏文本，把云联想 / 本地整句模型的异步结果主动推给
  对应会话。
- **IPC 协议**（`qingjian-platform::protocol`，两端共用）：`ClientMessage`（DLL → Server：开 / 关会话（带宿主 exe 名，
  `[apps]` 按应用设置据此查）、按键、上屏、回上下文）与 `ServerMessage`（Server → DLL：按键结果、上屏结果、异步重绘、请求上下文），一次要绘制的状态是
  `Frame`（preedit 分段 + 候选页）。长度前缀 JSON 帧的编解码与缺省管道名也在这里，DLL 不必依赖整个 Server 库。
- **TSF DLL**（`tsf/`）：分「引擎层」`client`（平台无关的管道客户端 `EngineClient`，泛型在任意 `Read + Write`
  上，本机就能接真 Server 端到端测）与「COM 层」`com`（`cfg(windows)`：`DllGetClassObject` → `IClassFactory`
  → `#[implement(ITfTextInputProcessor, ITfKeyEventSink)]`，编辑会话上屏，把组句位置报给 Server 摆候选窗口，
  `DllRegisterServer` 注册文本服务）。

设计细节见 `docs/design/architecture.md`「Windows：TSF」。

## 构建

本机（macOS / Linux）只做交叉 `check`，产出不了可用二进制，但协议层的端到端测试能跑：

```bash
cargo check --target x86_64-pc-windows-gnu -p qingjian-windows-server -p qingjian-windows-tsf
cargo test -p qingjian-windows-server -p qingjian-windows-tsf
```

真正编译与试用都在 Windows 机器上（MSVC 工具链）。

### 本地联调：`scripts/test-local.ps1`

改了 Server 或 DLL 之后要试手，用这个脚本，别手敲：

```powershell
pwsh -File apps\windows\scripts\test-local.ps1            # 编译 + 换 Server + 起，DLL 那步要管理员
pwsh -File apps\windows\scripts\test-local.ps1 -SkipBuild -SkipRegister
```

它做四件事：编 Server 与 TSF DLL（带 `QINGJIAN_UIACCESS=0`，没签名的 exe 带 uiAccess 起不来）、
停掉在跑的 Server、把**已安装目录整份拷到 `~\qingjian-devtest`** 再换上新的 Server 与随包数据、
起 Server 并打印剩下要手动做的事（注册 DLL、设方案、敲哪几组键）。

> **脚本存成带 BOM 的 UTF-8**（`sign-local.ps1` 也是）。Windows PowerShell 5.1 对没有 BOM 的 `.ps1`
> 按系统 ANSI 码页解析，中文会变成乱码、引号配对跟着崩，报出来的却是「字符串缺少终止符」这种语法错。
> 编辑时别把 BOM 去掉。PS7 两种都读得对，所以只用 `pwsh` 验会漏掉这个问题。

**为什么不在仓库里直接跑**：`bundled_root()` 按 exe 位置找数据，`target\debug\` 下会落到仓库根，
而 `data\generated\` 是 gitignore 的、本机多半没有，于是退回 `assets\sample\` 样例——形码候选照样出得来
（码表独立），但译文几乎全空，会让人误以为释义那条设计没生效。

**哪些更新需要一起换**：`qingjian_core::Candidate` 是线上格式的一部分（见 `protocol/mod.rs`）。
给旧 DLL 发送它不认识的枚举变体，会使整条帧解码失败，因此破坏线格式兼容性的修改必须更新两端，或按能力协商保留旧回复。
兼容的 Server、识别模型、浮窗更新不用重装 DLL；已有 DLL 在后台重启后自动重连。语音薄桥 v1 经能力字段协商，旧端继续走原协议。

### 按组件更新

当前全局语音入口完全在后台：更新 Server 即可让已打开及以后安装的普通软件使用，无需换宿主 DLL。上屏依赖控件明确提供可编辑且非密码状态；特殊自绘控件可能不支持，未签名 uiAccess 的本地构建不能覆盖管理员权限软件。基础 DLL 更新的重新加载要求仅适用于宿主内输入法功能。

`scripts/update-components.ps1` 在现有安装目录上按 SHA256 更新，并保留恢复副本。清单中只写需要交付的组件，每项为绝对 `path` 与 `sha256`；支持 `server`、`settings`、安装目录设置窗口图标 `appIcon`、`x64`、`x86` 和 TSF 注册图标 `icon`，两个 DLL 必须成对提供。`appIcon` 的目标固定为安装目录下的 `miracle.ico`；`icon` 的目标固定为 `%ProgramData%\Qingjian\qingjian.ico`，供机器级 TSF 注册读取。只更新后台时省略两个 DLL；更换任务栏/输入源图标时提供 `icon`。先检查计划，再应用：

```powershell
pwsh -File apps\windows\scripts\update-components.ps1 -ManifestPath C:\packages\qingjian\manifest.json -ResultPath C:\packages\qingjian\result.json -Plan
pwsh -File apps\windows\scripts\update-components.ps1 -ManifestPath C:\packages\qingjian\manifest.json -ResultPath C:\packages\qingjian\result.json
```

脚本仅为替换系统安装文件提权，随后由原权限进程重启后台。文件无变化则不重启；后台更新不修改 DLL 注册。基础 DLL 变化时，结果中的 `oldApps` 列出仍加载旧版的应用，保存工作并重新打开这些应用一次。脚本不关闭用户的聊天、浏览器或文档窗口。`ok` 表示文件与注册校验成功，交付前还须检查后台就绪和实际宿主上屏。

### 手工步骤（脚本里也在做，供对照）

```bat
:: 1) 编译出 DLL 与 Server
cargo build -p qingjian-windows-tsf -p qingjian-windows-server

:: 2) 注册 Miracle 文本服务（改 HKEY_CLASSES_ROOT，图标写到兼容路径 %ProgramData%\Qingjian\qingjian.ico，要管理员）
regsvr32 target\debug\qingjian_tsf.dll

:: 3) 起 Server（引擎在这里；没起时 DLL 吃掉字母键但没有候选，起来后下一键 / 下次聚焦自动重连）
cargo run -p qingjian-windows-server

:: 4) 在系统「语言 / 输入法」里应能看到「Miracle」，切到它，在任意输入框敲字
::    日志都在 %LOCALAPPDATA%\Qingjian\logs\：server.<日期>.log / tsf.<日期>.log / settings.<日期>.log（按天，留 7 天）

:: 反注册
regsvr32 /u target\debug\qingjian_tsf.dll
```

## 设置程序与 Windows App Runtime

`settings/`（`qingjian-settings.exe`）用 Windows Reactor（WinUI 3）画界面，是三个产物里唯一依赖 Windows App Runtime 的。
它的部署方式是**自包含**：`build.rs` 让 `windows-reactor-setup` 把 `Microsoft.WindowsAppSDK.Runtime` 的 MSIX 解到
`target\release\` 并按自包含标记嵌清单，安装包把这些文件装到 exe 同级——不依赖机器上装没装框架包。
Windows 10 上框架依赖的引导走不通（它要先调 Windows 11 才有的 `TryCreatePackageDependency`），
同一个 `build.rs` 还把这两个 API 改成延迟加载：否则它们会进 exe 的导入表，Windows 10 在加载期就起不来。
定位过程、上游 issue / PR 与取舍见 `docs/notes/windows-win10.md`；打包侧见 `apps/windows/installer/README.md`。

## 版本与发布

各平台壳版本号独立（见 `docs/notes/release.md`）：两个 package 的 `version` 各自写在自己的 `Cargo.toml`，
不跟 workspace 走；发布标签用 `windows-v<版本>`。
