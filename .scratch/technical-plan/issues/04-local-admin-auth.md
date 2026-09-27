# 实现唯一管理者账号、登录与会话

Status: done

## 目标

让唯一管理者通过本地账号密码访问私人管理端，完成初始化、登录、退出和密码重置，并使所有私人 API 具备可验证的身份边界。

依据：[技术实施规格](../spec.md)、[登录 ADR](../../../docs/adr/0006-local-account-postgresql-session.md)、[验收边界](../testing-plan.md)。

## 实施范围

1. 在服务器命令入口中提供唯一管理者初始化和密码重置，经 mise 任务执行；拒绝重复初始化，不提供公开注册或默认密码。重置后撤销全部已有 Session。
2. 使用 Argon2id 保存密码哈希。按目标机器能力验证哈希参数、并发限制、登录限流、Session 空闲和绝对有效期；失败响应不泄露账号是否存在。
3. 用 PostgreSQL 保存可过期、可撤销的服务端 Session。登录成功轮换会话标识，退出立即撤销当前会话；浏览器只持有不含身份资料的安全 Cookie。生产 Cookie 设置 `Secure`、`HttpOnly`、`SameSite`，不设置跨子域 `Domain`。
4. 实现登录、当前会话、退出及密码重置所需的 API；Rust 对每个私人请求鉴权，状态修改请求校验 CSRF token 和请求来源。接口 DTO 经任务 03 的契约链生成前端类型。
5. 管理端实现登录页、受保护页面与失效会话处理；通过管理子域同源 `/api` 请求后端。前端路由保护不替代 API 鉴权。
6. 为 HTTP 与真实 PostgreSQL 的主要认证路径编写集成测试，并覆盖必要的浏览器登录及退出流程；接入 `mise run ci`。

## 验收条件

- 初始化唯一管理者后，正确密码可登录并访问私人 API；未登录、错误密码和过期会话均被拒绝。
- 退出后原会话立即失效；重置密码后所有旧会话失效；重复初始化失败，数据库和普通日志均不含明文密码或会话值。
- 缺少有效 CSRF token 或来源不匹配的状态修改请求被拒绝；合法同源请求可完成，公开的进程存活接口不受私人鉴权影响。
- 管理页面在登录、刷新、会话过期和退出时显示正确状态；`mise run ci` 覆盖 API 与关键页面行为。
- 开发环境和生产 HTTPS 环境的 Cookie 与代理配置分别验证，不以关闭生产安全属性解决本地开发问题。

## 依赖与交接

- 依赖任务 01、02、03。
- 本任务不实现多管理者、第三方登录、邮件找回密码或个人管理业务 API。
- 完成时记录初始化与重置操作、配置项、验证结果和剩余风险，不记录真实凭据。

## 验收记录

- 2026-09-27：在 macOS arm64 使用 `mise run ci` 完整通过。真实 PostgreSQL 的隔离数据库测试覆盖唯一管理者初始化及拒绝重复初始化、Argon2id 哈希、未知账号与错误密码同响应、正确密码登录、Session 摘要存储和轮换、空闲与绝对过期、退出及密码重置撤销旧会话、登录限流、CSRF 缺失或错误、来源不匹配与公开存活接口。测试没有保存真实凭据。
- Chromium 浏览器测试覆盖未登录跳转、登录、刷新保持会话、会话过期和退出；同次测试经 Vite 同源 `/api` 代理访问实际 Rust 存活接口，确认路径转发。页面认证响应使用测试拦截，服务器行为由真实 PostgreSQL 集成测试独立验证。
- 开发模式 Cookie 经 HTTP 测试确认 `HttpOnly`、`SameSite=Strict`、`Path=/` 且无 `Secure`；HTTPS 配置经 API 测试确认 `__Host-` 前缀、`Secure`、`HttpOnly`、`SameSite=Strict`、`Path=/` 且无 `Domain`，非本机 HTTP origin 被拒绝。生产管理子域 Caddy 代理示例通过 `caddy:2.10.2-alpine` 的配置校验。
- `mise run contract:generate` 更新 Rust DTO 派生的 OpenAPI 与 TypeScript 类型；`mise run contract:check`、格式检查、Clippy、ESLint、Vue 类型检查、数据库测试、浏览器测试和双端构建由 `mise run ci` 通过。管理者初始化和重置入口分别为 `ADMIN_USERNAME=owner mise run admin:init` 与 `mise run admin:reset`，两者在终端交互式读取密码。
- [GitHub Actions Ubuntu Linux x64 检查](https://github.com/aifuxi/mindfolio/actions/runs/36320835553)通过，同一 `mise run ci` 执行数据库、契约、静态检查、构建与 Chromium 浏览器流程。

## 遗留问题与下一步

- 目标 VPS、实际管理子域和证书尚未提供，因此没有在目标生产 HTTPS 入口实测 Cookie、代理及 Argon2id 资源占用。上线前在目标机器设置 `AUTH_ORIGIN`、内部 `API_BIND`，按实际并发与资源测量并调整哈希参数、限流和 Session 有效期，再通过真实 HTTPS 浏览器完成登录与退出验收。
- 当前登录限流为单 API 进程的全局内存时间窗，适合首版单实例部署；若以后启用多实例，应迁移至共享存储并重新评估阈值。后续私人业务路由必须逐请求使用同一认证与写请求 CSRF/来源边界，不可只依赖前端路由保护。
