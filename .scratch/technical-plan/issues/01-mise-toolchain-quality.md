# 建立 mise 统一工具链与 Rust、前端质量检查

Status: ready-for-agent

## 目标

落实管理者确认的统一环境要求，为后续功能开发提供真实可运行的最小工程：所有项目命令经 mise 执行，Rust 使用 rustfmt、Clippy，Vue 与 TypeScript 使用 ESLint、Prettier，并在本地和 CI 执行相同检查。

依据：[技术实施规格](../spec.md)、[ADR 0011](../../../docs/adr/0011-mise-unified-toolchain.md)、[协作约定](../../../AGENTS.md)。当前仓库只有文档，本任务尚未实施。

## 实施范围

1. 验证兼容版本，初始化最小 Rust Cargo workspace、Vue 3 + TypeScript + Vite 管理端与 pnpm workspace，为实际源码配置质量工具。管理端接入已确认的组件库；本任务不实现业务功能或第二阶段 Nuxt 应用。
2. 在根目录建立 `mise.toml`，固定 Rust、Node.js、pnpm 的精确版本，Rust 使用 minimal profile 并显式安装 rustfmt、Clippy；固定并记录 mise 自身引导版本。生成并提交适用平台的 `mise.lock`，保留 `Cargo.lock`、`pnpm-lock.yaml`。避免同时手动维护不同的 Rust 版本来源。
3. 配置 `rustfmt.toml` 和适用的 Clippy 规则；Clippy 只在有明确项目规则时增加独立配置，不为生成配置文件而臆造阈值。Rust 格式检查调用 `cargo fmt --all -- --check`，lint 调用 `cargo clippy --workspace --all-targets --locked -- -D warnings`；这些调用放入 mise 任务内部。feature 组合根据真实工程选择，不机械启用所有 feature。
4. 为实际 Vue 与 TypeScript 源码配置 ESLint flat config，使用 Vue 和 TypeScript 的推荐规则；区分浏览器和 Node.js 环境。Prettier 独立运行，使用 `eslint-config-prettier` 关闭冲突格式规则并置于最后，不引入 `eslint-plugin-prettier` 重复检查格式。
5. 将 ESLint、Prettier、Vue 类型检查及相关插件作为 pnpm 开发依赖固定。配置合理的忽略范围，排除构建、缓存和生成目录，不忽略手写源码。Vue 单文件组件使用 TypeScript，类型检查独立于 Vite 构建。
6. 建立规格规定的 setup、dev、fmt、fmt:check、lint、typecheck、check、test、build、ci 任务。只为已建立的工程提供可运行任务；测试至少验证最小应用一个实际行为，不写占位测试或空成功脚本。fmt 可以修改格式，检查任务不修改源码；契约生成检查在契约链路建立时加入，并验证产物一致性。
7. mise 安装按锁定模式执行，pnpm 安装使用冻结锁文件，Cargo 命令在适用时使用锁定模式。任务依赖显式保证准备步骤先于检查；允许独立检查并行，但不能把并行依赖列表当作顺序执行。错误退出码必须逐层传递。
8. 配置 GitHub Actions 本地可复现的质量检查流程，复用 mise 版本、工具安装与 `mise run ci`。未建立 GitHub 远端前先验证同一流程的本地命令，不声称远端 CI 已通过。镜像发布、部署、备份恢复任务在对应阶段接入同一约定。
9. 更新开发说明和 `.gitignore`：首次引导后项目命令全部使用 `mise run` 或 `mise exec --`；解释首次信任配置、环境安装、检查与修复入口。本地环境覆盖文件和秘密信息不进入版本库。后续 Docker 构建复用锁定版本与任务，运行镜像仅携带所需依赖。

## 验收条件

- 在受支持的本地开发平台和 CI Linux 环境，能够通过锁定工具安装及 `mise run setup` 准备工程；记录实际验证平台，不能以单平台成功声称所有平台可用。
- 经 mise 验证 Rust、rustfmt、Clippy、Node.js 和 pnpm 版本与配置一致；缺少工具时明确失败或按规定完成安装，不静默依赖不匹配的全局版本。
- `mise run fmt:check`、`mise run lint`、`mise run typecheck`、`mise run check`、`mise run test`、`mise run build` 和 `mise run ci` 在真实最小工程中成功，输出与实际覆盖范围相符。
- 一次性引入可恢复的格式、Rust lint、Vue 或 TypeScript lint 与类型错误，分别确认对应入口返回失败，再还原源码；不为此添加镜像工具实现的永久测试。
- 检查不会自动修复源码；`mise run fmt` 可以修复格式。所有 lint 警告视为失败，构建成功不能掩盖独立类型检查失败。
- 开发服务能启动并访问，停止时正确退出；所有任务失败能传递至聚合任务和 CI，没有忽略错误或未实现阶段的空任务。
- 锁文件、配置、实际源码、任务和文档同步提交；不提交秘密信息，不安装或部署生产服务。

## 后续衔接

下一批工程基础工作建立契约生成和数据库迁移，并将对应验收加入 mise 任务。第二阶段引入 Nuxt 后使用其生成的 ESLint 配置，保留公共质量约定，确保 Nuxt 准备步骤先于 lint 和类型检查。

## 讨论

- 2026-09-27：管理者要求所有环境使用 mise，配置 Rust 与前端质量工具，并统一命令入口；据此列为首个工程基础任务。当前仅完成规格，不将计划中的配置视为已经运行验证。
