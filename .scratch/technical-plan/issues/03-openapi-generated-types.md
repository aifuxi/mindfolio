# 建立 OpenAPI 与 TypeScript 类型生成链

Status: done

## 目标

让 Rust 请求和响应 DTO 成为唯一接口结构来源，自动生成 OpenAPI 3.1.0 与管理端使用的 TypeScript 类型，并在 CI 发现契约漂移。

依据：[技术实施规格](../spec.md)、[契约 ADR](../../../docs/adr/0001-rust-dto-openapi-typescript.md)、[单仓库 ADR](../../../docs/adr/0008-monorepo-pnpm-workspace.md)。

## 实施范围

1. 在任务 01 的实际工具链上验证并锁定 `utoipa`、`openapi-typescript` 和所需的前端请求库版本；保持 OpenAPI 输出为 3.1.0。
2. 从已存在的真实最小 API 接口提取 Rust DTO，生成 OpenAPI 文档和共享 TypeScript 类型。数据库模型与对外 DTO 分离，不以手写 TypeScript 接口作为第二来源。
3. 明确 `/api` 路径、错误响应、请求标识、字符串 ID、日期和可空字段在生成链中的表示；尚未实现的业务字段只写契约约定，不造示例业务 API。
4. 提供 mise 生成与只读漂移检查任务，加入 `mise run ci`；生成步骤与前端类型检查的依赖顺序明确。前端至少有一个真实请求使用生成类型。
5. 文档说明新增或修改 API 时更新 DTO、生成产物和执行检查的操作顺序。

## 验收条件

- 修改真实 Rust DTO 后，重新生成能同步改变 OpenAPI 与 TypeScript 类型；生成产物与运行中的 HTTP 响应一致。
- 手工改坏或遗漏生成产物时，只读漂移检查失败，并提示生成命令；`mise run ci` 不自动修复源码或生成物。
- 前端类型检查确实使用生成类型；字符串 ID、日期、可空字段和错误响应的表示与实施规格一致。
- 工具和依赖版本已锁定；本地与 GitHub Actions 使用同一 mise 任务验证。

## 依赖与交接

- 依赖任务 01；可与任务 02 分别推进。若选择数据库就绪接口作为首个契约，先完成任务 02。
- 本任务不设计项目任务、习惯或公开网站的业务 DTO。
- 完成时记录生成工具版本、产物提交策略、生成命令及漂移检查结果。

## 验收记录

- 2026-09-27：在 macOS arm64 上锁定并验证 `utoipa` `6.0.0`、`openapi-typescript` `7.13.0` 与 `openapi-fetch` `0.17.0`。`mise run contract:generate` 从 Rust DTO 和路由注解生成 OpenAPI `3.1.0` 与共享 TypeScript 类型，两个产物均提交版本库；`mise run contract:check` 只读比较产物。
- 临时把真实 `HealthStatus.status` 序列化字段改为 `state` 后重新生成，OpenAPI 与 TypeScript 均同步出现 `state`；恢复 DTO 并重新生成后，产物回到 `status`。
- 手工破坏生成的 TypeScript 文件后，`mise run contract:check` 和 `mise run ci` 均返回非零并提示 `mise run contract:generate`；破坏内容仍在，确认检查未自动修复。临时改坏生成的路径类型后，独立 Vue 类型检查报错，证明管理端实际依赖生成类型。所有临时变更已还原。
- 完整 `mise run ci` 通过。通过本地 Vite 同源 `/api` 代理请求时，`/api/health/live` 与 `/api/health/ready` 均返回 HTTP 200 及 `{"status":"ok"}`；停库后就绪接口返回 HTTP 503，包含 `database_unavailable`、中文消息和 UUID 请求标识，与生成契约一致。开发服务和数据库容器已停止。
- [GitHub Actions Ubuntu Linux x64 检查](https://github.com/aifuxi/mindfolio/actions/runs/36319753019)通过，远端执行了相同的 `mise run ci` 与契约漂移检查。

## 遗留问题与下一步

- 当前真实接口没有资源 ID、业务日期或可空业务字段；已在 README 固定其表示约定，字段级生成验证须随首个实际使用这些字段的 API 完成，不添加虚构业务接口。本次尚未在目标生产代理环境验证 `/api` 转发。
- 后续独立任务 04 可使用生成类型实现管理者认证 DTO、接口及页面，并补充实际请求字段、错误响应和鉴权边界的契约验收。
