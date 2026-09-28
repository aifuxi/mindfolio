# 第一阶段部署与恢复入口

本手册对应任务 02 的配置交付。目标服务器发布与真实证书签发在任务 04 验证；服务器外备份在任务 03 建立。在完成备份与目标验收前，不把此配置视为已上线。

## 固定布局与版本

服务器预留 `/srv/mindfolio/repo` 作为仓库检出目录，`/srv/mindfolio/config/production.env` 存放权限为 `0600` 的真实变量，`/srv/mindfolio/edge/caddy/Caddyfile` 存放从仓库同步的入口配置。`DEPLOY_ENV_FILE` 默认指向上述变量文件；在其他路径部署时显式设置该变量。Caddy 挂载整个目录，更新配置后不会遇到单文件挂载指向旧 inode 的问题。

| 单元 | Compose | 固定 project | 持久卷及网络 |
| --- | --- | --- | --- |
| Portainer | `deploy/compose.portainer.yaml` | `mindfolio-portainer` | `mindfolio_portainer_data`；9443 只绑定 `127.0.0.1` |
| 入口 | `deploy/compose.edge.yaml` | `mindfolio-edge` | `mindfolio_edge_data`、`mindfolio_edge_config`、外部网络 `mindfolio_edge` |
| 应用 | `deploy/compose.app.yaml` | `mindfolio-app` | `mindfolio_postgres_data`、应用内网、外部网络 `mindfolio_edge` |

入口镜像由官方 Caddy 2.10.2 builder 加入 `caddy-dns/alidns` v1.0.29 构建，发布为 `ghcr.io/aifuxi/mindfolio-edge@sha256:...`。管理端镜像内的 Caddy 只提供静态文件及 `/api` 去前缀代理；入口 Caddy 将整个管理域名转发给它。所有生产镜像使用流水线给出的不可变 digest。当前镜像只构建 `linux/amd64`，目标服务器架构在任务 04 核对。

## 阿里云 DNS challenge

`deploy/edge.Caddyfile` 使用 `acme_dns alidns`。实际域名必须由阿里云云解析 DNS 托管权威解析，并为 `admin` 子域配置指向服务器的 A/AAAA 记录。为 Caddy 建立单独的 RAM 身份和 AccessKey，仅授予该域名完成 DNS 记录操作所需的 `DescribeDomains`、`DescribeDomainRecords`、`AddDomainRecord`、`UpdateDomainRecord`、`DeleteDomainRecord` 权限；按阿里云可用的资源条件继续缩小范围。密钥写在服务器的 `production.env` 和 Portainer Stack 环境变量中，不写进 Caddyfile、镜像或 Git。持有 Docker socket 管理权限的人能读取容器环境变量，因此只给受信任的管理员访问 Portainer 和服务器。

DNS challenge 通过 AliDNS API 创建临时 TXT 记录。验证前检查域名权威 NS、RAM 权限、Caddy 容器到阿里云 API 与公共 DNS 的出站连接，以及 443/TCP 的入站连通性；80/TCP 用于 HTTP 到 HTTPS 跳转，443/UDP 用于 HTTP/3。`/data` 保存证书和私钥，`/config` 保存 Caddy 状态，恢复时沿用原卷名并按任务 03 的备份方案恢复。当前本地检查仅证明模块存在、配置可解析，不代表真实签发和续期成功。相关配置依据：[AliDNS 模块](https://github.com/caddy-dns/alidns)、[libdns 权限说明](https://github.com/libdns/alidns)、[Caddy DNS challenge](https://caddyserver.com/docs/caddyfile/options#acme_dns)。

## 首次准备

1. 在服务器安装 Docker Engine、Compose 插件及固定版本的 mise；检出仓库到 `/srv/mindfolio/repo`，执行 `mise trust`。确认服务器是 `linux/amd64`，且 80/443 未被其他服务占用。
2. 创建 `/srv/mindfolio/config` 和 `/srv/mindfolio/edge/caddy`，将 `deploy/production.env.example` 复制为 `production.env` 并设置 `0600`，将 `deploy/edge.Caddyfile` 复制为 `/srv/mindfolio/edge/caddy/Caddyfile`。填入真实管理子域、精确的 `AUTH_ORIGIN=https://管理子域`、AliDNS RAM 凭据、随机数据库密码及三个已发布的镜像 digest。`DATABASE_URL` 中的密码需要 URL 编码，且与 `POSTGRES_PASSWORD` 相同。不要用样例值启动生产环境。
3. 用 `mise run deploy:check` 检查配置，再运行 `mise run deploy:network:create` 创建外部网络。保留 `production.env` 的服务器外安全副本；后续备份任务会覆盖入口状态及 Portainer 状态。
4. `mise run deploy:portainer:up` 启动面板。通过 `ssh -N -L 9443:127.0.0.1:9443 用户@服务器` 建立隧道，再访问 `https://localhost:9443` 完成首次初始化。面板证书初始为自签名。
5. 在 Portainer 创建应用 Stack。首次 API 启动会自动执行版本化迁移；确认 API 与 PostgreSQL 健康后，运行 `mise run deploy:migrate` 再次核对独立迁移入口。使用 `ADMIN_USERNAME=实际用户名 mise run deploy:admin:init` 在交互终端输入两次密码；密码不通过参数或环境文件传递。再创建入口 Stack，运行 `mise run deploy:status`，并从浏览器验证 `https://管理子域/` 和 `https://管理子域/api/health/ready`。

Portainer 中分别建立名为 `mindfolio-app`、`mindfolio-edge` 的 Stack。选择 Git Repository，仓库为 `https://github.com/aifuxi/mindfolio.git`，Compose 路径分别为 `deploy/compose.app.yaml`、`deploy/compose.edge.yaml`，关闭自动 GitOps 更新。将受保护的 `production.env` 中对应变量导入 Stack 环境变量，保持与服务器恢复文件一致。入口 Caddyfile 是宿主机绝对路径挂载；Portainer CE 不会仅凭 Git 仓库中的相对文件自动把它放到宿主机。首次发布直接由 Portainer 创建 Stack；`deploy:db:up`、`deploy:app:up`、`deploy:edge:up` 留作面板外恢复，不能先从 CLI 启动再重复创建 Stack。Portainer 的 Git Stack 与环境变量用法见[官方文档](https://docs.portainer.io/user/docker/stacks/add)。

## 发布与迁移

1. 在通过 `mise run ci` 的提交上读取 GitHub Actions `publish` 摘要，记录完整提交号及 API、管理端、入口镜像 digest。镜像 tag 用于追溯，部署变量使用 `name@sha256:...`。先核对数据库备份有效恢复点。
2. 对照新旧镜像所含迁移脚本评估兼容性。当前最新迁移为 `202609270011_weekly_review.sql`；仅已部署的同一 API 版本及其同一迁移集合经过验证，**未承诺更早镜像可读取迁移后的数据库**。如果新迁移可能破坏旧程序，先安排维护窗口，停止旧 API 与管理端，再迁移；若是经过验证的向后兼容迁移，可保持服务运行。镜像回退不等于数据库回滚，需从备份恢复才能回到旧模式及数据状态。
3. 在 `production.env` 中把 `API_IMAGE`、`ADMIN_IMAGE` 改为同一提交的新 digest，运行 `mise run deploy:migrate`。这会用新镜像中的独立 `migrate` 程序处理现有数据库；失败时停止发布并核对数据库状态。API 启动时也会执行幂等迁移，但这不替代发布前的显式步骤。
4. 在 Portainer 的 `mindfolio-app` Stack 更新 `API_IMAGE`、`ADMIN_IMAGE` 环境变量为 `production.env` 中的 digest，手动重新部署。确认健康状态、登录、Cookie、CSRF 和日志，并核对面板变量与服务器文件一致。如果入口镜像或 Caddyfile 变化，单独更新 `mindfolio-edge` Stack；Caddyfile 单独变化可同步宿主机文件后用 `mise run deploy:edge:reload` 验证并重载。入口容器重建会造成短暂中断。

API、PostgreSQL 没有公网 `ports`，PostgreSQL 只连应用内网，API 只连应用内网；管理端桥接应用内网和入口网络，入口只连外部共享网络。每个服务有 Docker 日志轮转、CPU/内存上限与停止宽限期。2 核 4 GB 的初始限额需要在任务 04 按实际峰值复核，避免数据库或 API 被 OOM 杀死。查看日志可运行 `mise exec -- docker compose --env-file /srv/mindfolio/config/production.env -f deploy/compose.app.yaml logs --tail 100 api`；不要运行会打印秘密的 `docker compose config`，只用 `config --quiet`。

## 面板或入口故障

SSH 是独立恢复入口。Portainer 域名或 Caddy 不可用时，使用上述 SSH 隧道直连 `127.0.0.1:9443`；面板容器故障时运行 `mise run deploy:portainer:up`。入口故障时先检查 `/srv/mindfolio/edge/caddy/Caddyfile`、AliDNS 权限、DNS TXT 传播、`mindfolio_edge_data` 卷及 `mise run deploy:status`，再用 `mise run deploy:edge:reload` 或 `mise run deploy:edge:up` 恢复。

面板无法操作 Stack 时，在原 `/srv/mindfolio/repo`、原 `production.env` 与原 Caddy 挂载路径下运行 `mise run deploy:network:create`、`mise run deploy:db:up`、`mise run deploy:migrate`、`mise run deploy:app:up`、`mise run deploy:edge:up`。Compose 文件中的 project 名、服务名和卷名保持不变，避免生成空数据库或新的证书状态。恢复面板后核对 Stack 记录与 Docker 现状；不要直接删除卷或重建同名 Stack。Portainer 数据、Caddy `/data` 与 `/config`、PostgreSQL 数据的服务器外备份步骤见[离站备份与恢复点](./backup.md)，实际恢复演练由任务 05 完成。
