# 使用 mise 统一工具环境与命令入口

管理者明确要求所有环境使用 mise 管理，后续统一通过 mise 运行项目命令。固定 Rust、Node.js、pnpm 和 mise 自身版本，本地开发、测试、CI、容器构建与服务器运维复用版本约定及任务入口；Cargo 和 pnpm 继续负责各自依赖，Docker 继续管理容器服务。Rust 配置 rustfmt 与 Clippy，前端配置 ESLint 与 Prettier，格式修复与只读检查分开，lint 警告使检查失败。

该决定使环境重建与检查行为集中维护。配置和锁文件在真实最小工程建立后验证提交，不为尚未实现的应用创建空检查；生产仅准备运行及运维所需依赖，启动时不下载开发工具链。具体任务与验收见[工程基础任务](../../.scratch/technical-plan/issues/01-mise-toolchain-quality.md)。
