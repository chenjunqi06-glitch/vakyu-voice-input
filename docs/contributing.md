# 开发约定

改代码前先看这一页：架构上不能越的线、代码怎么组织、版本号与提交信息怎么写、改了行为要同步哪些文档。
设计的来龙去脉在 [design/architecture.md](design/architecture.md)，各 crate 的实现要点在 [notes/crate-notes.md](notes/crate-notes.md)。

## 架构约束（不要违反）

长版与理由见 [design/architecture.md](design/architecture.md)。

- **Core 与平台层严格解耦。** `qingjian-core` 及其兄弟 crate 必须平台无关：词库、拼音解析、候选生成、排序、学习、翻译、文本变换全部属于 Core。
  平台层（IMK / TSF / IBus-Fcitx）只做两件事：把系统输入事件翻译成 Core 的输入，把 Core 返回的帧画到候选窗口。
  **平台层里不允许出现排序逻辑、词库访问、翻译调用或文本变换。** 判断标准：把 IMK 换成 TSF，不应该需要改 Core 的任何一行。
- **一个候选词只显示一种辅助语言。** 用户配置 Primary Language + 单个 Learning Language。不要设计成 `translations: Vec<Translation>` 或
  `HashMap<Lang, String>` 这类多语言并列的数据结构，那会在 API 层面把「一次只学一种语言」这条产品原则给破坏掉。翻译是候选词的 annotation（可选、单条）。
- **输入优先于学习。** 任何为学习功能增加的延迟、弹窗、UI 干扰都是设计错误。翻译查询不能阻塞候选生成，Core 必须能在翻译尚未就绪时先返回候选。
- **输入方案是配置项，不是模式。** 双拼、注音这类键盘方案放 `[general]` 里当设置，中 / 英切换始终是布尔；新方案不能改变别的方案的既定按键行为（[user/getting-started/keys.md](user/getting-started/keys.md)）。
- **显示面自绘、控件面原生。** 候选窗、拼音行、状态条这类显示面由渲染器出位图各平台贴图（主题靠它）；偏好设置、菜单、安装器用各平台原生控件。见 [design/rendering.md](design/rendering.md)。

## 代码组织

- **一个类型一个文件。** 一个 struct / enum / trait 及其 impl 单独一个文件；模块文件只做 `mod` 声明、re-export 与自由函数，不把一个 crate 平铺在 `lib.rs` 里。
- **子模块用目录。** 有子模块的模块用 `foo/mod.rs`，**不用** `foo.rs` + `foo/` 并列。
- **同词干的兄弟文件收进目录，绝不用文件名前缀分组。** `key_event.rs` + `key_outcome.rs` → `key/mod.rs` + `key/{event,outcome}.rs`，哪怕没有 `key.rs` 这个共同父文件；
  `query/` + `querying.rs` 这种也不行。判断：两个及以上文件名共享一段前缀且同属一个概念，就收进以那段前缀命名的目录。
- **一个职责连带它专用的类型收进一个目录。** `engine/commit/mod.rs` 放上屏部分，`chain.rs` / `last.rs` 放只有它用的类型。
- **大类型的 `impl` 按职责拆成子模块。** 每个文件一个 `impl Foo { … }`（`host/settings.rs` 这样），结构体与构造留在 `mod.rs`，跨文件用到的私有方法标 `pub(super)`。
- **文件长度。** 单文件不超过 800 行，目标 500 行以内；测试超过 200 行搬到 `tests.rs`（多时 `tests/` 按主题分文件）。
- **导入写精确路径。** 不用 `use super::*` / `use foo::*`（`#[cfg(test)] mod tests` 里的 `use super::*` 除外）。
  子文件从父模块拿的类型写 `use super::{Engine, Candidate};`，父模块没定义的直接 `use crate::…` 或 `use <crate>::…`，不绕 `super::` 转手。
  存量的 glob 改到那个文件时顺手换掉，不专门清扫。

## 命名与注释

- 代码标识符一律英文，注释与文档用中文；`thiserror` 的 `#[error]` 文案用英文，日志与 UI 文案用中文。
- 新文件都要有 `//!` 文件头；结构体 / 枚举字段之间空一行，字段名说不清的加 `///`（`r` / `g` / `b`、`width` 这类不用，`a`「255 为不透明」这类要）。
- 注释只写维护时用得上的：文件头一两句说它是什么、怎么用；代码里只解释读代码看不出来的约束与原因（平台怪癖、协议约定、踩过的坑），一两句说完。
  不写复述代码的教学性注释，不写改动经过、调试过程这类日志型注释；较长的设计理由写进 `docs/`，注释里留一句指过去。
- 不写装饰性分隔注释（`// ====`），提交钩子会拦。

## 依赖与配置

- 依赖用 `cargo add` 加，共用包提到根 `[workspace.dependencies]`；错误用 `thiserror` 不用 `anyhow`；日志用 `tracing` 门面。
- 快捷键一律进 `[shortcut]` 可配置，不写死键码。

## 版本号

- `crates/*` 用 `version.workspace = true`；**`apps/*` 各壳是独立发布的产品，写死自己的 `version`**（Windows 读 `server/Cargo.toml`）。
- 发版之间带 `-dev`，打包脚本可再接 git 短哈希（脏加 `+`，Cargo.toml 里只写 `-dev`）；Windows 公共安装包也可通过 `-PackageVersion` 指定本轮构建标识。
- Vakyu 当前以 GitHub 预发布版交付测试构建，标签和附件版本必须能对应到同一份源码。开发标签应明确标为预发布，不宣称稳定版；pkg / Inno 的内部版本仍使用数字点号。当前没有由标签自动构建或上传安装包的工作流。

## 提交信息

- [Conventional Commits](https://www.conventionalcommits.org/zh-hans/)：第一行 `<类型>(<范围>): <说明>`，类型与范围英文小写，说明用中文，例如
  `fix(core): 修自绘输入框吞数字`、`feat(windows): 三进程日志统一到 %LOCALAPPDATA%\Qingjian\logs`、`docs(changelog): 补 0.1.3 条目`。
  - 类型：`feat` 新功能 / `fix` 修 bug / `docs` 只改文档 / `refactor` 不改行为的整理 / `perf` 性能 / `test` 只改测试 /
    `build` 打包与构建脚本 / `ci` 工作流 / `chore` 版本号、依赖、仓库杂务 / `style` 只改格式 / `revert` 还原。
  - 范围：crate 或壳的名字——`core` `platform` `render` `dictionary` `translate` `learning` `predict` `lm` `neural` `format` `cli`
    `macos` `windows`（Server / DLL / 设置程序细分时用 `server` `tsf` `settings`）`installer` `linux` `tools` `docs` `ci` `deps` `release`；
    跨好几处的可以省略。不兼容的改动在范围后加 `!`。
  - 正文写「为什么」与取舍，一行一条；不加 AI 署名。`.githooks/commit-msg` 会拦第一行不合格式的提交。
  - 2026-09-16 之前的历史是「`macOS：……` / `Core：……`」的中文冒号格式，不重写。

## 文档同步

- 实现与规划分歧时以代码为准并改文档。技术方向写 `docs/design/`，计划写 `docs/plan/`，工程记录写 `docs/notes/`；不往 README 里加技术内容。
- **用户能感知的行为改了（按键、菜单、偏好设置、配置文件、数据文件），同一个提交里改 `docs/user/` 对应的页**，按键改动同时改 `keys.md`。
- 改了某个 crate 的实现要点（数据文件、常数、生成命令），同步 `docs/notes/crate-notes.md`。

## 提交前检查

- 钩子：`.githooks/pre-commit`（禁装饰性分隔注释 + fmt + clippy）、`.githooks/commit-msg`（提交信息格式）、`.githooks/pre-push`（全 workspace 测试）；`git config core.hooksPath .githooks` 启用一次。
  两个跑编译的钩子按 `uname` 划平台范围：**非 Apple 平台排除 `qingjian-macos`**（IMK 壳依赖 objc2，在别的平台上是硬 `compile_error!`，
  排除不掉就整条命令失败）。可选 [ci.yml](github-workflows/ci.yml) 明确选择 Windows 与共享核心 package，macOS 另有移植工作流示例；Windows 上 pre-push 另设 `QINGJIAN_UIACCESS=0`
  （Server 的 build.rs 嵌 uiAccess manifest，没签名的测试二进制起不来，os error 740）。
- 排序 / 整句 / 纠错的改动先跑 `apps/cli` 再合。

## CI 示例与发版

当前公开仓库未启用 GitHub Actions。下列流程仅作为 `docs/github-workflows/` 中的可选示例提供；事件和执行步骤描述的是启用后的行为，并非已运行的验收记录。

- [ci.yml](github-workflows/ci.yml) 在 main 推送、PR 和手动触发时检查 Windows x64 构建、Server / TSF / 设置单元测试、共享核心测试及 x86 TSF 编译检查。Rust 命令使用 `--locked`；未签名测试使用 `QINGJIAN_UIACCESS=0`。Python 使用 3.12 x64，并从语音运行时的固定版本 requirements 安装依赖，再运行不使用真实麦克风的单元测试。
- [macos-port.yml](github-workflows/macos-port.yml) 由相关路径 PR 或手动触发，在 Apple Silicon runner 上尝试构建移植包与执行检查；没有成功的运行记录和真机验收，不得宣称 macOS 已可用。[audit.yml](github-workflows/audit.yml) 保留每周与锁文件变更的 RustSec 审计，Dependabot 维护 actions 版本。
- Vakyu 安装包目前按 [Windows 打包与校验流程](../apps/windows/installer/README.md) 构建，人工上传本仓库 Release，同时提供对应源码、许可及 SHA-256 清单。不得打包开发者密钥、个人数据或本机自签信任证书；不得将青简上游附件当作 Vakyu 安装包。
- [release.yml](github-workflows/release.yml) **仅手动验证已发布附件**：输入标签，下载当前仓库该 Release 的文件并校验 `SHA256SUMS.txt`。它不自动构建、不上传附件，也不证明全新 Windows 安装或真实语音交互通过。
- CHANGELOG 手写、发版时由维护者统一改（PR 不动它）。[旧发版笔记](notes/release.md)保留青简上游来源与历史流程，不能把其中的上游仓库、Secrets、标签触发机制直接套用到当前 Vakyu 发布。

## 外部 PR

- 从 main 开分支，一个 PR 只做一件事、只碰一个平台（Core 改动单独一个）。
- 维护者对着 main 审，squash 合并保留作者署名。PR 模板里的合并前清单就是审核标准。
- **修 bug、改壳里行为（按键、上屏、候选窗位置）的 PR 必须真机验过**：先在自己机器上复现问题，改完在同一个应用里确认修好，
  PR 的「怎么验证的」写明系统版本、应用与操作步骤。编译与 CI 通过不算验证。没验过的修复发出去，报 issue 的人升级后还得再报一次。
  复现不了的（没有那个应用或系统）不提修复：把分析写在 issue 里，或者提只加日志、不改行为的 PR。
  不改行为的改动（日志、注释、文档）与有测试 / 回放兜底的 Core 逻辑不受此限。
- 主题与自绘渲染器（`crates/qingjian-render`、各壳的贴图路径、主题文件）还在测试，这部分暂不接受 PR；稳定一版后再开。
