# 建立 OpenAPI 与 TypeScript 类型生成链

Status: ready-for-agent

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
