# 项目协作约定

- 使用中文沟通；新增文档与代码注释使用中文，技术名词、命令、路径和配置键名保留原文。
- Git 提交消息使用中文，并遵循 Conventional Commit 规范。
- 开展工程开发时，按需初始化或更新 codegraph，并用它了解项目结构与代码关系。

## 工具与执行

### 环境与统一命令

- 本地开发、测试、CI、镜像构建和服务器运维统一使用 mise 管理项目工具环境及命令入口；Rust、Node.js、pnpm 和 mise 自身版本在工程初始化时验证并固定。
- 常规操作使用 `mise run <任务>`；尚未封装的临时命令使用 `mise exec -- <命令>`，包括通过 IDE 执行的 shell 命令。mise 任务内部可调用 Cargo、pnpm 等底层工具，日常操作与文档不另设绕过 mise 的入口。首次安装 mise 本身属于环境引导。
- 工程初始化后提交 mise 配置与锁文件，依赖安装遵守各自锁文件。本地秘密信息不进入版本库；Docker 继续管理容器服务，部署和恢复的命令行操作经 mise 任务执行，Portainer 面板仍用于已确认的人工版本发布。
- Rust 必须配置 rustfmt 和 Clippy；前端必须配置 ESLint 和 Prettier。ESLint 负责代码质量，Prettier 负责格式，通过配置关闭冲突的格式规则。
- 统一提供 `mise run fmt`、`mise run fmt:check`、`mise run lint`、`mise run typecheck`、`mise run check`、`mise run test`、`mise run build` 和 `mise run ci`。只有 `fmt` 自动修复格式，检查与 CI 不自动修复；lint 警告视为失败。
- 尚未初始化的应用不得用跳过检查或空任务伪装通过；任务随真实工程建立，命令失败必须向上传递。工具链缺失时先通过 mise 准备环境，再运行检查。

### MCP 使用顺序

- 代码定位、依赖分析和调用链调查优先调用 `mcp__codegraph__codegraph_explore`，传入当前仓库根目录的绝对 `projectPath`；索引过期时运行 `mise exec -- codegraph sync .`。
- 文件读取、项目诊断、运行配置和 IDE 执行：优先调用 `mcp__rustrover__*`，并始终传入当前仓库的绝对 `projectPath`。
- 判断 MCP 是否可用时，不得只检查首屏或显式展开的工具列表；必须先检查完整工具目录，包括延迟加载工具（运行时提供 `ALL_TOOLS` 或等价工具搜索时必须使用）。
- 只有完整工具目录中不存在对应工具，或找到工具后实际调用失败，才可声明 MCP 不可用并回退 CLI；不得将“首屏未显示”视为“未注册”。

## Agent skills

### 任务跟踪

本项目使用 `.scratch/` 下的本地 Markdown 文件记录规格和任务，详见 `docs/agents/issue-tracker.md`。

### 领域文档

本项目采用单一上下文：术语记录在根目录 `CONTEXT.md`，架构决策记录在 `docs/adr/`，详见 `docs/agents/domain.md`。这些文件在出现已确认的术语或决策时创建。

### 项目级 skills

本项目选用的 Matt Pocock skills 位于 `.agents/skills/`。仓库已通过 `setup-matt-pocock-skills` 完成初始配置；需求澄清使用 `grill-with-docs`，讨论成熟后使用 `to-spec`。`grilling` 和 `domain-modeling` 是 `grill-with-docs` 的依赖。仅在切换任务跟踪方式或重建配置时重新运行 setup。
