# 建立生产镜像与 GHCR 发布流水线

Status: completed

## 目标

让 API 与管理端拥有可在目标服务器拉取、可追溯到提交和 digest 的生产镜像。

依据：[本批规格](../spec.md)、[技术实施规格](../../technical-plan/spec.md)。

## 实施范围

1. 分别建立 Rust API 与管理端静态服务的生产镜像。构建阶段遵守锁定依赖与 `mise` 任务；运行阶段仅保留服务需要的文件与依赖，管理端静态服务维持 `/api` 代理约定。
2. 提供经 `mise run` 调用的本地镜像构建与验证入口；把镜像构建纳入适用的 CI 检查，失败使流水线失败。
3. 扩展 GitHub Actions：检查通过后在受控分支或版本事件发布 GHCR 镜像，记录提交、版本与不可变 digest；PR 只检查，不发布。避免把令牌写入镜像、日志或仓库。
4. 说明架构支持、镜像标签与 digest 的选择方式，以及迁移程序如何随版本获得，不在目标服务器编译。

## 验收条件

- 两个镜像可从锁定源码和依赖构建；API 容器能提供健康检查，管理端能加载静态页面并把 `/api` 请求转发至 API。
- CI 对构建失败如实失败；受控发布产生两个可拉取的 digest，并能查到对应提交；PR 不获得生产发布能力。
- 镜像中没有构建时凭据、开发依赖或默认管理员密码；`mise run ci` 与镜像验证通过。

## 依赖与交接

- 依赖已完成的第一阶段业务与现有 CI。GHCR 可见性和目标 CPU 架构在发布前确认。
- 向任务 02 交付镜像名称、digest 引用格式、运行配置与迁移命令。
- 本任务不安装目标服务器，也不执行 Portainer 发布。

## 验收记录

- 2026-09-28：本地 `mise run image:build` 已构建 Linux x64 的 API 与管理端镜像；`mise run image:verify` 使用隔离 PostgreSQL 验证健康检查、静态页面、`/api` 转发和独立迁移程序，结束后清理临时容器与数据卷。
- `mise run ci`、脚本语法检查与 Git 差异检查通过；两个运行镜像均使用 `10001:10001` 用户，并带有构建提交标识。
- GitHub Actions [运行 36362046622](https://github.com/aifuxi/mindfolio/actions/runs/36362046622) 的 `ci` 与 `publish` 均通过；`master` 提交 `f0218a139b1e6d69ec9fdba5a127624f67a29a4d` 发布版本标签 `sha-f0218a139b1e6d69ec9fdba5a127624f67a29a4d`。
- API 不可变引用：`ghcr.io/aifuxi/mindfolio-api@sha256:04cdd0075f23d3dba071b15f5c03c80f8a7e9a74644edf7fcfd86a0517455c74`；管理端不可变引用：`ghcr.io/aifuxi/mindfolio-admin@sha256:8eb703b3e98c352075a8fce80297163a9045deb5e5710c27086508c1d070cff9`。使用空 Docker 凭据配置读取两个远端 manifest 均成功，确认可拉取。
- 流水线对 PR 仅授予 `contents: read`，发布 job 仅在 `master` 的 push 后运行并单独取得 `packages: write`；未向镜像传入 GHCR 令牌。目标服务器架构尚未确认，当前交付平台为 `linux/amd64`。
