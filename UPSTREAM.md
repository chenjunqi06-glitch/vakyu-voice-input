# 上游、许可与项目关系

Vakyu 是在 [Qingjian / 青简](https://github.com/qingjian-team/qingjian) 基础上继续迭代的独立修改版，并非青简官方发行版。感谢青简原作者及主要维护者 [@yenharvey](https://github.com/yenharvey)、qingjian-team 与[所有贡献者](https://github.com/qingjian-team/qingjian/graphs/contributors)。上游作者、贡献者及各文件中的版权声明继续保留。

- 初始上游源码快照：[c08ae57cb88b6a4a46f4a5e9c1d6d11c5e69222e](https://github.com/qingjian-team/qingjian/commit/c08ae57cb88b6a4a46f4a5e9c1d6d11c5e69222e)。
- Vakyu 修改与发布方：本仓库维护者；首次公开修改版于 **2026-10-10** 发布，后续修改日期与内容由 Git 提交历史记录。
- 原作者身份依据：青简[首个公开版本提交](https://github.com/qingjian-team/qingjian/commit/7a2ccc769183403de4c8ccfd8fbd72fc96ed9579)及上述来源快照的提交署名；该致谢同时涵盖社区贡献，不表示单一作者拥有所有第三方内容。

致谢、链接及作者提及不表示上游为 Vakyu 提供认可、担保或用户支持。Vakyu 的问题与兼容性反馈请提交到本仓库，避免把独立修改版的问题转交上游。

在上游输入法核心之上，Vakyu 增加和维护 Windows 全局语音输入、中英双向口语跟读、发音容错、逐词进度、离线词卡、快捷键配置、托盘入口、主题以及包含语音运行时的 Windows 安装包。

代码沿用 **GPL-3.0-or-later**，完整条款见 [LICENSE](LICENSE)。分发修改后的二进制时，需同时按许可证要求提供对应源码及构建信息。GitHub Release 附带的 `source.zip` 是对应修改源码归档，另含锁定 Rust 依赖的源码；网页自动生成的 Source code 压缩包不含这些依赖附件。

随包模型、词库、字体、Python、微软运行库及其他依赖分别适用各自许可证，不因为合并进安装器就统一改成 GPL。具体声明安装在程序目录的 `licenses/`，来源与固定版本见 `apps/windows/installer/voice-sources.json`、`voice-models.json`、`assets/` 中的说明以及 [数据来源](docs/design/landscape.md)。

目录、crate、注册标识及部分兼容路径保留 `qingjian`，以兼容旧安装和用户数据。上游网站链接仅用于来源、历史设计与致谢，不是 Vakyu 安装包或客服入口。本项目不会自动向上游仓库、官网或发布索引推送更新。

项目名称、logo 及第三方商标不因代码许可而授予商标权；请勿将自己的修改版宣传成上游或 Vakyu 官方发行版。
