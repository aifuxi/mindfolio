# Mindfolio

本仓库先建立 Rust API 与 Vue 管理端的最小工程。API 提供进程存活接口 `GET /health/live` 和数据库就绪接口 `GET /health/ready`；契约生成与认证将在后续任务实现。

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
| `mise run db:start` / `mise run db:stop` | 启动或停止本地 PostgreSQL，停止时保留数据卷 |
| `mise run db:status` | 查看数据库容器状态 |
| `mise run db:migrate` | 对开发数据库执行版本化迁移 |
| `mise run db:prepare:test` | 准备独立的测试数据库 |
| `mise run db:verify:empty` | 从空库执行迁移并重复执行核对 |
| `mise run fmt` | 修复 Rust 与前端格式 |
| `mise run fmt:check` | 只读格式检查 |
| `mise run lint` | Clippy 和 ESLint；警告也失败 |
| `mise run typecheck` | Vue 和 TypeScript 类型检查 |
| `mise run check` | 汇总格式、lint、类型检查 |
| `mise run test` | Rust 与前端测试 |
| `mise run build` | 构建两个应用 |
| `mise run ci` | 执行全部检查、测试与构建 |

未封装的临时命令使用 `mise exec -- <命令>`。开发服务按 `Ctrl+C` 停止。管理端通过 Vite 将 `/api` 代理到本地 API。API 启动时连接数据库并执行迁移；连接或迁移失败会使启动失败。运行中数据库不可用时，`/health/ready` 返回 503，而 `/health/live` 仍表示进程存活。

## 本地数据库

首次运行 `mise run db:start` 会生成仅供本地使用的 `.env.db`，其中包含随机数据库密码，文件已被 Git 忽略。PostgreSQL `17.7-alpine` 使用固定镜像 digest，默认仅监听本机 `55432` 端口；开发库为 `mindfolio_dev`，测试库为 `mindfolio_test`。可在首次启动前通过 `.env.db` 的 `DB_PORT` 修改端口。测试库独立于开发库；测试用事务在结束时回滚。

数据库异常时先运行 `mise run db:status`，再通过 `mise exec -- docker compose --env-file .env.db -f compose.db.yaml logs postgres` 查看容器日志；迁移失败会由 `mise run db:migrate` 输出错误。`mise run db:verify:empty` 使用临时数据库核对空库、重复迁移及结构冲突时报错，并在结束后删除临时库。开发数据卷不会随 `db:stop` 删除。

本地 `.env`、`mise.local.toml` 与其他秘密信息不进入版本库。新增工程阶段时，先接入真实任务与检查，再扩展 `mise run ci`。
