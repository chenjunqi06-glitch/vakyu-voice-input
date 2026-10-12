# macOS 安装后的输入源登记核验

2026-10-11 的本机排查确认，仅原地替换主二进制/Info.plist 并签名，可能使两个 TIS 输入源 ID 在新进程里查询为 NULL；回滚文件本身也不能恢复登记。必须从最终安装路径重新运行 `--register`。这与界面成功启动或日志显示 IMK 进程运行是不同的验收项。

生产安装路径存在两个遗漏：开发 `bundle.sh --install` 用 `--register || true` 吞掉失败；pkg 的 `postinstall` 在登记/启动失败后仍然以 0 退出。旧的 `--register` 本身已有新进程回读启用状态与延迟重查，但它没有核对输入源实际资源路径，也未检查仍存在的同 ID 应用副本。

本次增加 `scripts/verify_input_source.m`。它只读 TIS 与 LaunchServices，在全新进程确认：

- 父输入源和可见模式均恰好找到一项，bundle ID 符合安装包；
- 二者已启用，模式可选择；不强制抢占当前焦点或输入源；
- 两者图标资源实际来自传入的最终安装路径，文件可读；
- 用户和系统的 `Library/Input Methods` 目录没有其他仍存在的同 ID 安装路径；工作区构建产物不算已安装副本。

核验器在构建时以目标架构编译，随外层 app 签名，安装用户无需 Xcode 或 Swift。它不修改 TIS、TCC、输入文字或应用配置。`register_input_source.sh` 只接受用户级/系统级的最终 Vakyu.app 路径，先验签，再向 LaunchServices 登记，并执行已安装二进制的 `--register`，随后在两个独立进程里核验，间隔 3 秒；如果系统后续重扫覆盖，最多重试完整过程一次，持续失败返回非零。

开发安装迁入 `install_local.sh`，复制和验签成功后交换目录，暂存旧包。登记、启动或最终核验失败时恢复旧包并重新登记；启动项也恢复原文件。首次安装失败时先通过已保存的原输入源 ID 恢复选择，再注销并清理本次包；若恢复选择失败，保留本次应用供恢复，防止删除仍被选中的输入法。pkg 的 preinstall 在不停止旧进程的情况下备份旧包/启动项、拒绝用户/系统重复安装；postinstall 完成最终目录登记与核验，失败时恢复原安装与登记。没有登录的桌面用户时在载荷写入之前失败，因为不能完成该用户的真实 TIS 回读。系统安装中断发生在 preinstall 之后而 postinstall 之前时，备份会保留，下一次安装明确拒绝覆盖它，必须先检查该次恢复状态。

构建产物改为 `target/macos-bundle.noindex/Vakyu.app`，pkg 载荷使用 `.noindex` 目录，并在生成组件 pkg 后删除临时 payload app。排除自动索引可减少构建副本被系统误登记。LaunchServices 发现工作区副本并不说明 TIS 正在使用它；核验仍通过父项/模式的实际图标 URL 确认 TIS 指向最终安装目录。不要用未知路径的全量注销来清理，注销同 ID 副本后仍须登记最终安装路径。

语音启动项升级在 `bootout` 后最多等待 10 秒，确认旧服务条目已移除后再 `bootstrap`。遇到系统尚在清理的瞬时注册失败最多尝试三次；持续失败仍交由安装器回滚。2026-10-12 本机升级最初在直接 `bootstrap` 时返回 EIO，加入等待后构建 8 安装成功，最终输入源核验通过，语音进程保持运行。

验证记录：

- Objective-C 核验器通过 `clang -fobjc-arc -Wall -Wextra -Werror -mmacosx-version-min=13.0` 构建。
- 2026-10-11 00:33 的独立进程只读核验在当前真实安装上成功；沙盒内无法访问完整 TIS 服务，须在正常登录用户环境验证，不能用受限工具的 NULL 查询判定系统登记失效。
- `apps/macos/tests/test_install_scripts.py` 的 20 个测试通过。在隔离目录模拟成功、短暂重扫、持续登记失败、验签失败、冲突安装、最后回读失败、pkg 原名称回滚和旧名称回滚、首次安装失败恢复先前输入源、备份失败清理，不操作真实输入源。输出见 `target/validation/install-script-tests.log`。
- shell 语法检查与 `check_port.py` 源码检查通过。
- 本子任务没有执行真实安装、登记、注销或重启语音宿主；这次脚本改动尚未做真实 pkg 升级验收。

本核验只确认安装与登记状态。真实 TextEdit/微信/B站的候选显示、中文上屏、Fn 切换及语音填入仍必须分别验收，不能由 TIS 回读结果代替。

## 运行期自愈（2026-10-12）

安装时的核验只覆盖安装那一刻。之后原地升级、回滚或系统重扫仍可能让两个输入源 ID 查询为 NULL：此时微信等应用选中 Vakyu 会立刻跳回，按 Fn 弹出输入法浮窗时系统 `TUINSCursorUIController._selectCurrentInputSource` 会对空输入源 `CFRelease` 而崩溃。现在两处会自动补登记：

- 主输入法进程启动时（`input_source::heal_registration`）：自己的输入源查不到就对本包调一次 `TISRegisterInputSource`。只处理 `Input Methods` 目录里的 `.app`，不启用、不切换。
- 语音宿主（`InputSourceGuard`）：登录后 5 秒、每 30 秒、唤醒与切回会话后各查一次父项和 Hans 模式。查不到才登记外层 `Vakyu.app`，两次修复至少间隔 10 分钟；仅当 `com.apple.inputsources` 的 `AppleEnabledThirdPartyInputSources` 里原本有 Vakyu 时才重新启用，从不切换当前输入源。构建副本（`.noindex`）不会被登记。

日志关键字：`输入源在系统里查不到，已重新登记`（主进程）、`Vakyu input source repaired`（语音宿主）。这只恢复登记；微信聊天框能否组字仍须按上文分流诊断验收。

