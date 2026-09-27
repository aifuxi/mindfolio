# 项目协作约定

- 使用中文沟通；新增文档与代码注释使用中文，技术名词、命令、路径和配置键名保留原文。
- Git 提交消息使用中文，并遵循 Conventional Commit 规范。
- 开展工程开发时，按需初始化或更新 codegraph，并用它了解项目结构与代码关系。

## Agent skills

### 任务跟踪

本项目使用 `.scratch/` 下的本地 Markdown 文件记录规格和任务，详见 `docs/agents/issue-tracker.md`。

### 领域文档

本项目采用单一上下文：术语记录在根目录 `CONTEXT.md`，架构决策记录在 `docs/adr/`，详见 `docs/agents/domain.md`。这些文件在出现已确认的术语或决策时创建。

### 项目级 skills

本项目选用的 Matt Pocock skills 位于 `.agents/skills/`。仓库已通过 `setup-matt-pocock-skills` 完成初始配置；需求澄清使用 `grill-with-docs`，讨论成熟后使用 `to-spec`。`grilling` 和 `domain-modeling` 是 `grill-with-docs` 的依赖。仅在切换任务跟踪方式或重建配置时重新运行 setup。
