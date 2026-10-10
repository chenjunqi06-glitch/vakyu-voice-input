# 可选 GitHub Actions 工作流示例

首次公开发布仅上传源码和安装包，**没有启用自动 CI 或自动发布**。这些 YAML 文件保存在普通文档目录，不会在推送、PR 或定时事件中执行。

如维护者以后决定启用，可按需复制到仓库根的 `.github/workflows/`。通过 OAuth / Token 推送工作流通常还需相应的 `workflow` 权限。启用前检查 runner、依赖版本与权限；本机单元测试通过不代表这些工作流已在 GitHub 执行成功。

| 文件 | 用途 |
|---|---|
| `ci.yml` | Windows 构建、共享 Rust 核心测试、Python 语音测试和 x86 TSF 检查 |
| `release.yml` | 手动下载本仓库已发布附件，校验 `SHA256SUMS.txt`；不会自动发布 |
| `macos-port.yml` | Apple Silicon 移植构建与检查，仍需 Mac 真机验收 |
| `audit.yml` | RustSec 依赖审计；包含写入检查和 Issue 权限，外部 fork PR 受 GitHub 权限限制 |

原青简工作流另保留在 `docs/upstream-workflows/` 供查阅。它们也不执行，不应直接用于向上游或其官网推送。
