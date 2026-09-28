# Mindfolio

本仓库包含 Rust API 与 Vue 管理端。API 提供健康检查和唯一管理者认证；管理端使用 Rust DTO 生成的接口类型。

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
| `ADMIN_USERNAME=owner mise run admin:init` | 交互式初始化唯一管理者 |
| `mise run admin:reset` | 交互式重置密码并撤销全部旧会话 |
| `mise run db:prepare:test` | 准备独立的测试数据库 |
| `mise run db:verify:empty` | 从空库执行迁移并重复执行核对 |
| `mise run contract:generate` | 从 Rust DTO 生成 OpenAPI 3.1.0 与 TypeScript 类型 |
| `mise run contract:check` | 只读检查已提交的契约产物是否同步 |
| `mise run fmt` | 修复 Rust 与前端格式 |
| `mise run fmt:check` | 只读格式检查 |
| `mise run lint` | Clippy 和 ESLint；警告也失败 |
| `mise run typecheck` | Vue 和 TypeScript 类型检查 |
| `mise run check` | 汇总格式、lint、类型检查 |
| `mise run test` | Rust 与前端测试 |
| `mise run build` | 构建两个应用 |
| `mise run ci` | 执行全部检查、测试与构建 |
| `mise run image:build` | 构建 Linux x64 的 API、管理端与 AliDNS 入口镜像 |
| `mise run image:verify` | 在隔离容器中验证镜像、AliDNS 模块、健康检查与同源代理 |
| `mise run deploy:check` | 检查生产 Compose 配置；需设置 `DEPLOY_ENV_FILE` |
| `mise run deploy:verify` | 隔离验证入口代理与服务网络边界 |

未封装的临时命令使用 `mise exec -- <命令>`。开发服务按 `Ctrl+C` 停止。管理端通过 Vite 将 `/api` 代理到本地 API。API 启动时连接数据库并执行迁移；连接或迁移失败会使启动失败。运行中数据库不可用时，`/health/ready` 返回 503，而 `/health/live` 仍表示进程存活。

## 本地数据库

首次运行 `mise run db:start` 会生成仅供本地使用的 `.env.db`，其中包含随机数据库密码，文件已被 Git 忽略。PostgreSQL `17.7-alpine` 使用固定镜像 digest，默认仅监听本机 `55432` 端口；开发库为 `mindfolio_dev`，测试库为 `mindfolio_test`。可在首次启动前通过 `.env.db` 的 `DB_PORT` 修改端口。测试库独立于开发库；测试用事务在结束时回滚。

数据库异常时先运行 `mise run db:status`，再通过 `mise exec -- docker compose --env-file .env.db -f compose.db.yaml logs postgres` 查看容器日志；迁移失败会由 `mise run db:migrate` 输出错误。`mise run db:verify:empty` 使用临时数据库核对空库、重复迁移及结构冲突时报错，并在结束后删除临时库。开发数据卷不会随 `db:stop` 删除。

## 接口契约

修改 API 时，先在 Rust 请求或响应 DTO 与对应的 `#[utoipa::path]` 注解中更新结构，再运行 `mise run contract:generate`，提交 `packages/api-contract/openapi.json` 和 `packages/api-contract/src/schema.d.ts`。随后运行 `mise run contract:check` 与 `mise run ci`。漂移检查只读，失败时提示重新生成；管理端通过 `@mindfolio/api-contract` 和 `openapi-fetch` 使用生成类型。

OpenAPI 的 `servers` 为同源 `/api`，文档 `paths` 为后端路由；管理端请求 `/api/auth/session` 等接口，本地 Vite 代理去掉 `/api` 后转发给 Rust，生产代理需保持相同约定。错误响应包含稳定 `code`、中文 `message` 和字符串 `request_id`。认证契约声明管理端 Cookie、浏览器同源 `Origin` 要求及退出请求的 `x-csrf-token`。

后续业务 DTO 的资源 ID（含参数和关联引用）使用十进制字符串，不输出 JSON 大整数；业务日期使用 `YYYY-MM-DD` 字符串，实际时刻使用 UTC RFC 3339 字符串。可空字段应在 Rust 序列化和 OpenAPI 中明确表示 `null`，与字段缺省区分。当前健康接口没有业务 ID、日期或可空字段，这些字段的生成结果须在首个实际使用它们的 API 任务中验证。

本地 `.env`、`mise.local.toml` 与其他秘密信息不进入版本库。新增工程阶段时，先接入真实任务与检查，再扩展 `mise run ci`。

## 管理者认证

先运行 `mise run db:start`，再用 `ADMIN_USERNAME=owner mise run admin:init` 初始化唯一账号。命令在终端两次交互式读取密码，不接受默认密码；密码长度为 12 至 1024 字节。重复初始化明确失败。遗忘密码时在服务器终端执行 `mise run admin:reset`，两次输入新密码，成功后全部旧 Session 立即失效。不要把密码写入命令参数、环境文件或普通日志。

本地开发用 `mise run dev` 打开 `http://127.0.0.1:5173`。Vite 通过同源 `/api` 转发请求，开发 Cookie 为仅本机使用的 `mf_session`，设置 `HttpOnly`、`SameSite=Strict` 和 `Path=/`。生产环境必须设置 `AUTH_ORIGIN=https://admin.example.com`（替换为实际管理子域）；API 拒绝非本机 HTTP origin。生产 Cookie 名为 `__Host-mf_session`，额外设置 `Secure`，不设置 `Domain`。浏览器仅持有不含身份资料的随机会话值，数据库只保存其 SHA-256 摘要；密码用 Argon2id 哈希。登录成功轮换已有会话，退出撤销当前会话，重置撤销全部会话。私人请求由 Rust 查询 Session；写请求须携带当前会话的 `x-csrf-token` 并通过 `Origin` 校验。

| 配置项 | 默认值 | 用途 |
| --- | --- | --- |
| `AUTH_ORIGIN` | 必填；本地 mise 任务设为 `http://127.0.0.1:5173` | 管理端精确同源地址及 Cookie 安全模式 |
| `API_BIND` | `127.0.0.1:3001` | API 监听地址；容器内按受控网络设置为 `0.0.0.0:3001` |
| `AUTH_IDLE_SECONDS` | `1800` | Session 空闲有效期 |
| `AUTH_ABSOLUTE_SECONDS` | `604800` | Session 绝对有效期 |
| `AUTH_HASH_MEMORY_KIB` | `19456` | Argon2id 内存成本，最小 19 MiB |
| `AUTH_HASH_ITERATIONS` | `2` | Argon2id 迭代次数 |
| `AUTH_HASH_CONCURRENCY` | `2` | 同时执行密码验证的最大任务数 |
| `AUTH_LOGIN_MAX_ATTEMPTS` | `5` | 单进程、全局时间窗内的登录次数上限 |
| `AUTH_LOGIN_WINDOW_SECONDS` | `60` | 登录限流时间窗 |

生产部署参见 [部署与恢复入口](docs/deployment.md)：入口 Caddy 使用 AliDNS DNS challenge，为管理子域提供 HTTPS；管理端镜像处理静态文件和 `/api` 去前缀代理。目标 VPS 的 CPU、内存、代理和 Cookie 实测需在部署前完成；本地与 CI 的安全属性检查不能代替目标环境验收。

## 生产镜像

`mise run image:build` 用锁定的 `mise`、Rust、Node.js、pnpm、Cargo 与 pnpm 依赖构建 `mindfolio-api:local` 和 `mindfolio-admin:local`，并构建带 AliDNS 模块的 `mindfolio-edge:local`。当前镜像只支持 `linux/amd64`；在 macOS arm64 上构建与验证会使用容器模拟执行。构建环境使用 Debian 13，API 运行镜像只保留 API、`migrate`、`admin` 程序及健康检查需要的系统依赖；管理端运行镜像只包含 Caddy 与静态产物。API 和管理端均以非 root 用户运行，镜像中不设置管理员密码。

`mise run image:verify` 创建临时 PostgreSQL、API 和管理端容器，检查数据库就绪、静态页面、同源 `/api` 转发、独立 `migrate` 程序，以及入口镜像的 AliDNS 模块与 Caddyfile 语法。`mise run deploy:verify` 再用生产 Stack 配置隔离检查入口路由与网络边界。两项任务结束后删除临时容器和数据卷。CI 在 `mise run ci` 通过后执行镜像构建与验证；只有 `master` 的 push 才进入具有 `packages: write` 权限的 GHCR 发布任务，PR 不发布。

GHCR 镜像分别为 `ghcr.io/aifuxi/mindfolio-api`、`ghcr.io/aifuxi/mindfolio-admin` 与 `ghcr.io/aifuxi/mindfolio-edge`，版本标签使用完整提交号 `sha-<Git SHA>`。流水线摘要记录提交、版本标签和三个 `name@sha256:<digest>` 引用；部署时使用 digest 固定镜像。API 镜像内的 `/usr/local/bin/migrate` 和 `/usr/local/bin/admin` 与服务程序来自同一构建版本。Stack 配置、迁移顺序和手动发布步骤见[部署与恢复入口](docs/deployment.md)。
