# Mindfolio

本仓库先建立 Rust API 与 Vue 管理端的最小工程。当前 API 仅提供进程存活接口 `GET /health/live`；数据库就绪、契约生成与认证将在后续任务实现。

## 环境准备

首次安装 [mise](https://mise.jdx.dev/) 时使用版本 `2026.6.14`。克隆仓库后，在根目录审阅并信任 `mise.toml`，再执行：

```sh
mise trust
mise install --locked rust node pnpm
mise run setup
```

已验证的环境为 macOS arm64 和 GitHub Actions Ubuntu Linux x64。配置固定 Rust `1.97.1`（minimal profile，额外安装 rustfmt 与 Clippy）、Node.js `24.21.0`、pnpm `12.3.4`。`mise.lock` 包含两个平台的下载记录。Cargo 和 pnpm 依赖分别使用 `Cargo.lock` 与 `pnpm-lock.yaml`。

## 常用命令

| 命令 | 用途 |
| --- | --- |
| `mise run dev` | 同时启动 API 与管理端，访问 `http://127.0.0.1:5173` |
| `mise run fmt` | 修复 Rust 与前端格式 |
| `mise run fmt:check` | 只读格式检查 |
| `mise run lint` | Clippy 和 ESLint；警告也失败 |
| `mise run typecheck` | Vue 和 TypeScript 类型检查 |
| `mise run check` | 汇总格式、lint、类型检查 |
| `mise run test` | Rust 与前端测试 |
| `mise run build` | 构建两个应用 |
| `mise run ci` | 执行全部检查、测试与构建 |

未封装的临时命令使用 `mise exec -- <命令>`。开发服务按 `Ctrl+C` 停止。管理端通过 Vite 将 `/api` 代理到本地 API。仅有存活接口不表示数据库已就绪。

本地 `.env`、`mise.local.toml` 与其他秘密信息不进入版本库。新增工程阶段时，先接入真实任务与检查，再扩展 `mise run ci`。
