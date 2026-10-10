# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 仓库现状

Vakyu 是基于 [Qingjian / 青简](https://github.com/qingjian-team/qingjian) 的独立开源衍生项目，沿用 GPL-3.0-or-later 与上游版权声明。Core 平台无关，各平台负责系统集成。
当前发布目标为 Windows 10 22H2 / Windows 11 原生 x64 测试版，新增本地语音与双向中英跟读；Apple Silicon macOS 移植尚需原生构建及真机验收，iOS 暂无版本。Linux 保留上游 Fcitx5 输入法代码，新增语音功能未验证。
仓库中的旧版本、路线和发版记录可能描述青简上游；Vakyu 当前用户说明以 `README.md` 与 `docs/user/getting-started/vakyu-quickstart.md` 为准。当前未启用自动 CI；可选工作流示例与人工发版方式见 `docs/contributing.md`。

## 目录地图

一行一个，只说它是什么、入口在哪；实现要点（数据文件、常数、生成命令）在 `docs/notes/crate-notes.md`，改了实现要同步那里。

- `crates/qingjian-core`：引擎。`Engine` 是对外唯一门面，`Translator` / `Learner` 等 trait 在 `engine` 模块；拼音解析、纠错、候选、排序、整句、双拼、注音、英文模式都在这里。
- `crates/qingjian-dictionary`：词库（TSV 或 `.qj` mmap），按音节位置二分查询。
- `crates/qingjian-translate`：释义表 `Glossary`、词汇等级表 `LevelTable`。
- `crates/qingjian-learning`：用户侧落盘：词频 / 用户词 / 个人 n-gram / 敲错表（`FrequencyLearner`）、输入日志（`InputLog`）、输入统计（`UsageStats`）、词汇记录（`VocabularyBook`）。
- `crates/qingjian-predict`：云联想 `CloudPredictor`（OpenAI 兼容接口）与释义兜底 `CloudGlossFiller`；`PredictConfig` 是 `[predict]` 分节。
- `crates/qingjian-lm`：整句转换的 bigram 语言模型 `BigramModel`。
- `crates/qingjian-neural`：字级 Transformer 本地推理 `CharScorer`（candle），给整句前几条路径重打分。
- `crates/qingjian-format`：`.qj` 数据容器（mmap 读、零拷贝视图、写入器、哈希索引）。
- `crates/qingjian-platform`：平台层共用：`Config`（TOML 配置）、`extra_dictionaries`、Windows Server ↔ DLL 的 `protocol` 类型。
- `crates/qingjian-render`：自绘渲染器（spike 中，分支 renderer-spike）：候选窗一帧 + 主题 → 位图，各平台只贴图。见 `docs/design/rendering.md`。
- `crates/qingjian-update`：检查更新：读官网的 `releases.json`、验 ed25519 签名、按平台与渠道挑新版本，只提示不安装；发版侧的签名工具是 `tools/release-sign`。见 `docs/design/update.md`。
- `apps/cli`：Core 的验证工具：查询、逐键计时、输入日志回放、整句评测、常数扫描。排序 / 整句 / 纠错的改动先跑它再合。
- `apps/macos`：IMK 壳，按 `app / host / imk / candidates / menubar / preferences` 分目录；`scripts/bundle.sh --install` 装到本机，`--pkg` 出分发包。
- `apps/windows`：`server`（Server 进程：Engine + IPC + 自绘候选窗与状态条）+ `tsf`（TSF DLL）+ `settings`（WinUI 3）+ `installer`（Inno）。DLL 不能带 Engine 的依赖树，所以是两个 package。
- `apps/linux`：`server`（Engine + Unix socket）+ `fcitx5`（框架事件、默认面板）+ `scripts`（用户级安装卸载、`package.sh` 打预编译包）；Server 手动启动，见 `docs/notes/linux-fcitx5.md`。
- `tools/dict-convert`、`tools/gloss-gen`、`tools/corpus`：产品数据生成（词库 / 语言模型 / 释义表 / emoji / 英文词表），输出到 `data/generated/`（gitignore）。
- `assets/`：随包数据源与样例，各目录有 README 写来源与许可。雾凇拼音（GPL）已彻底移除，不要再引入。

`docs/` 分四类（索引在 `docs/README.md`）：`design/` 设计与决定、`plan/` 路线与待办、`notes/` 工程记录（性能、复盘、踩坑、crate 实现要点）、
`user/` 用户文档（官网构建时拉取渲染，约定见 `docs/user/README.md`，措辞面向用户、不出现实现词）。

## 常用命令

```bash
cargo build                                   # 整个 workspace
cargo test                                    # 全部测试；-p <crate> 单个，加测试名过滤
cargo clippy --all-targets -- -D warnings
cargo fmt --all
cargo run -p qingjian-cli -- <拼音>...          # Core 的主要验证方式；--replay / --eval-text 见 crate-notes
apps/macos/scripts/bundle.sh --install        # mac 壳装到 ~/Library/Input Methods/（IMK 不能 cargo run 验证）
```

Windows 使用 MSVC 编译 Server、TSF 和设置程序；测试与公开构建设置 `QINGJIAN_UIACCESS=0`，避免未签名测试程序要求提权。按平台选择 package，不能在 Windows 直接构建依赖 Apple 框架的 macOS 壳。部署方式见 `apps/windows/README.md`，完整分发包见 `apps/windows/installer/README.md`。
`docs/github-workflows/ci.yml` 提供 Windows 与共享核心检查示例；`macos-port.yml` 提供 Apple Silicon 移植检查示例。当前没有启用自动工作流。编译、单元测试或 CI 通过不代表麦克风、权限、跨应用填入已完成真机验收。

## 约定

架构约束、代码组织、版本号、提交信息、文档同步、提交前检查与发版都在 `docs/contributing.md`，随本文件一起载入：

@docs/contributing.md

交流用中文。
