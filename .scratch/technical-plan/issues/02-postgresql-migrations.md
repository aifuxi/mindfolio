# 建立 PostgreSQL 开发环境与版本化迁移

Status: done

## 目标

让最小 Rust API 连接真实 PostgreSQL，并能从空数据库重复建立所需结构，为唯一管理者登录和后续业务模块提供迁移基础。

依据：[技术实施规格](../spec.md)、[主键 ADR](../../../docs/adr/0002-postgresql-identity-primary-keys.md)、[登录 ADR](../../../docs/adr/0006-local-account-postgresql-session.md)。

## 实施范围

1. 在任务 01 建立的 mise 入口下，配置本地 Docker PostgreSQL 的启动、停止、状态查询和测试数据库准备；固定经过验证的镜像版本。开发与测试数据库隔离，不在版本库中保存真实凭据。
2. 为 API 接入 SQLx PostgreSQL 连接池、连接超时和清晰的启动错误。区分进程存活与数据库就绪；数据库不可用时就绪检查失败。
3. 建立版本化迁移。首批迁移只创建唯一管理者与服务端 Session 所需的真实表、主外键、唯一性和必要索引；业务主键使用 `BIGINT IDENTITY`，敏感凭据仅以必要的哈希值保存。不为通过迁移验收而添加无用途占位表。
4. 通过 mise 任务执行迁移和空库验证；处理迁移文件变化后的重新编译或重新执行问题，使本地与 CI 行为一致。
5. 用真实 PostgreSQL 验证空库迁移、重复执行、约束和 API 就绪响应。数据库相关测试接入 `mise run test` 与 `mise run ci`，失败向上传递。

## 验收条件

- 从空数据库执行统一任务后，管理者与 Session 表及约束存在；再次执行不产生重复结构或静默跳过错误。
- API 在数据库正常时返回就绪，在连接失败或迁移失败时明确报告失败；进程存活与数据库就绪的语义可区分。
- 测试数据库与日常开发数据隔离；测试能验证唯一管理者约束、会话关联及必要索引或约束行为。
- 所有数据库启动、迁移和测试命令经 mise 入口运行，`mise run ci` 真实执行数据库相关检查。
- 提交锁定的镜像引用、迁移、配置示例和运行说明，不提交密码或测试数据卷。

## 依赖与交接

- 依赖任务 01；完成后任务 04 可使用迁移结果实现认证。
- 本任务不实现登录 API、密码重置或个人管理业务表。
- 完成时记录空库准备、迁移、测试和故障诊断命令，以及实际验证环境。

## 验收记录

- 2026-09-27：在 macOS arm64、Docker Engine `29.4.0`、Docker Compose `5.1.2` 上，使用固定 digest 的 PostgreSQL `17.7-alpine`。`mise run db:start` 生成本地随机密码并启动容器，开发库与测试库分别为 `mindfolio_dev`、`mindfolio_test`。
- `mise run db:verify:empty` 从临时空库创建管理者和 Session 表，重复迁移后仍只有一条成功迁移记录；预置冲突表时，迁移命令以“数据库迁移失败”返回非零。临时验证库已删除。
- `mise run ci` 通过格式、Clippy、ESLint、类型、构建和测试。真实 PostgreSQL 集成测试验证唯一管理者约束、Session 外键与 token 唯一性、必要索引和就绪响应；开发库迁移由 `mise run db:migrate` 通过。
- 启动 `mise run dev:api` 后，`/health/live` 与 `/health/ready` 均返回 200；执行 `mise run db:stop` 后，前者仍为 200，后者返回 503。API 已停止，数据库容器保持停止，数据卷保留。

## 遗留问题与下一步

- 本地与 CI 共用 `mise run ci` 入口；本次尚未取得包含这项改动的 GitHub Actions 运行结果，也未在目标 VPS 验证。Docker 容器仅在本机完成实际运行验收。
- 后续按独立任务推进 03 OpenAPI 类型生成；04 管理者认证可在 03 完成后使用这里的表与迁移入口。首次初始化管理者、密码哈希和会话业务逻辑均属于 04。
