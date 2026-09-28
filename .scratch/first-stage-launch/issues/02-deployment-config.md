# 编写 Portainer、Caddy 与应用部署配置

Status: in-progress

## 目标

形成可审查、可复现的第一阶段部署配置和升级、故障处理步骤。

依据：[本批规格](../spec.md)、[部署 ADR](../../../docs/adr/0005-caddy-portainer-deployment.md)、[部署调研](../../technical-plan/deployment-research.md)。

## 实施范围

1. 提供服务器 Compose 管理的 Portainer 配置、独立 Caddy Stack 和应用 Stack 的版本化样例；确定固定目录、卷、外部网络、服务名与环境变量来源。Portainer 9443 仅绑定回环地址；数据库只在内部网络，API 不直接发布公网端口。
2. 入口 Caddy 配置管理端 HTTPS、同源 `/api` 代理与证书持久化；管理端静态服务保持独立镜像。给出域名、`AUTH_ORIGIN`、镜像 digest、数据库凭据等非秘密示例与实际配置清单。
3. 将数据库迁移、管理者初始化、健康检查、日志轮转、优雅退出、资源限制与手动版本选择写成可执行的 `mise` 任务或操作手册。明确迁移先后顺序和旧镜像兼容范围；失败处理不假定切换镜像可回滚数据。
4. 记录 SSH 隧道访问 Portainer、从面板外恢复原 Stack 与 Caddy 状态的步骤，保证恢复时沿用原 project、卷、网络和挂载路径。

## 验收条件

- 配置通过语法检查；在隔离 Docker 环境启动后，管理端、API 和数据库的网络边界与 `/api` 路由符合约定。
- 不填写秘密的样例足以生成实际部署配置；秘密不进入版本库、容器镜像或普通日志。
- 使用文档中的步骤可确定迁移与发布顺序、当前镜像 digest、健康状态和 SSH 恢复入口；`mise run ci` 通过。

## 依赖与交接

- 依赖任务 01 给出的镜像与迁移入口。实际域名及服务器路径在任务 04 接入时填入。
- 本任务交付部署配置和演练步骤，不将本地验证冒充目标服务器验收。

## 实施与验收记录

- 2026-09-28：管理者确认实际域名的权威 DNS 由阿里云托管，要求入口 Caddy 镜像支持 DNS challenge。入口镜像固定 Caddy 2.10.2 与 `caddy-dns/alidns` v1.0.29，通过 GHCR 流水线发布；阿里云 RAM 凭据留在部署环境。
- 已交付 Portainer、入口、应用三个 Compose 配置，Caddyfile、非秘密变量样例、`mise` 运维任务及 `docs/deployment.md`。入口、管理端、API、数据库分别按共享网络和应用内网连接；Portainer 9443 仅绑定回环地址。
- 本地 `mise run image:build`、`mise run image:verify`、`mise run deploy:verify`、`DEPLOY_ENV_FILE=deploy/production.env.example mise run deploy:check`、`mise run ci` 均通过。隔离验证确认 AliDNS 模块、Caddyfile 语法、入口 `/api` 去前缀代理及网络边界；`mise run ci` 包含 13 个浏览器测试。目标域名实际证书签发、目标服务器架构与资源占用仍属于任务 04。
- 待 GitHub Actions 发布并取得入口镜像不可变 digest 后完成本任务记录。
