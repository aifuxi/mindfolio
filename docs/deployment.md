# 第一阶段部署与恢复入口

本手册对应任务 02 的配置交付和任务 04 的实际部署。目标服务器使用 Debian 12，真实证书、应用健康及离站备份已在目标机验证；隔离恢复演练仍由任务 05 完成。

## 固定布局与版本

服务器预留 `/srv/mindfolio/repo` 作为仓库检出目录，`/srv/mindfolio/config/production.env` 存放权限为 `0600` 的真实变量，`/srv/mindfolio/edge/caddy/` 存放从仓库同步的 `Caddyfile` 与 `sites/*.caddy`。`DEPLOY_ENV_FILE` 默认指向上述变量文件；在其他路径部署时显式设置该变量。Caddy 挂载整个目录，更新配置后不会遇到单文件挂载指向旧 inode 的问题。

| 单元 | Compose | 固定 project | 持久卷及网络 |
| --- | --- | --- | --- |
| Portainer | `deploy/compose.portainer.yaml` | `mindfolio-portainer` | `mindfolio_portainer_data`、专用代理网络 `mindfolio_portainer_proxy`；9443 只绑定 `127.0.0.1` |
| 入口 | `deploy/compose.edge.yaml` | `mindfolio-edge` | `mindfolio_edge_data`、`mindfolio_edge_config`、外部网络 `mindfolio_edge` 与 `mindfolio_portainer_proxy` |
| 应用 | `deploy/compose.app.yaml` | `mindfolio-app` | `mindfolio_postgres_data`、应用内网、外部网络 `mindfolio_edge` |

入口镜像由官方 Caddy 2.10.2 builder 加入 `caddy-dns/alidns` v1.0.29 构建，发布为 `ghcr.io/aifuxi/mindfolio-edge@sha256:...`。管理端镜像内的 Caddy 只提供静态文件及 `/api` 去前缀代理；入口 Caddy 将整个管理域名转发给它。所有生产镜像使用流水线给出的不可变 digest。当前镜像只构建 `linux/amd64`，目标服务器架构在任务 04 核对。

## 阿里云 DNS challenge

`deploy/edge.Caddyfile` 定义共享的 `alidns_tls` 配置，并导入 `deploy/sites/*.caddy`。各站点经 `tls` 指令使用 `dns alidns`，DNS TXT 传播等待为 15 分钟，以容纳解析缓存。实际域名必须由阿里云云解析 DNS 托管权威解析，并为 `admin` 和 `portainer` 子域配置指向服务器的 A/AAAA 记录。为 Caddy 建立单独的 RAM 身份和 AccessKey，仅授予该域名完成 DNS 记录操作所需的 `DescribeDomains`、`DescribeDomainRecords`、`AddDomainRecord`、`UpdateDomainRecord`、`DeleteDomainRecord` 权限；按阿里云可用的资源条件继续缩小范围。密钥写在服务器的 `production.env` 和 Portainer Stack 环境变量中，不写进 Caddyfile、镜像或 Git。持有 Docker socket 管理权限的人能读取容器环境变量，因此只给受信任的管理员访问 Portainer 和服务器。

DNS challenge 通过 AliDNS API 创建临时 TXT 记录。验证前检查域名权威 NS、RAM 权限、Caddy 容器到阿里云 API 与公共 DNS 的出站连接，以及 443/TCP 的入站连通性；80/TCP 用于 HTTP 到 HTTPS 跳转，443/UDP 用于 HTTP/3。`/data` 保存证书和私钥，`/config` 保存 Caddy 状态，恢复时沿用原卷名并按任务 03 的备份方案恢复。目标机已取得正式证书，但续期仍需依靠后续运行时观察。相关配置依据：[AliDNS 模块](https://github.com/caddy-dns/alidns)、[libdns 权限说明](https://github.com/libdns/alidns)、[Caddy DNS challenge](https://caddyserver.com/docs/caddyfile/directives/tls)。

## 首次准备

1. 在服务器安装 Docker Engine、Compose 插件及固定版本的 mise；检出仓库到 `/srv/mindfolio/repo`，执行 `mise trust`。确认服务器是 `linux/amd64`，且 80/443 未被其他服务占用。
2. 创建 `/srv/mindfolio/config` 和 `/srv/mindfolio/edge/caddy`，将 `deploy/production.env.example` 复制为 `production.env` 并设置 `0600`，运行 `mise run deploy:edge:config:sync` 同步 `Caddyfile` 与站点文件。填入真实管理子域、Portainer 子域、精确的 `AUTH_ORIGIN=https://管理子域`、AliDNS RAM 凭据、随机数据库密码及三个已发布的镜像 digest。`DATABASE_URL` 中的密码需要 URL 编码，且与 `POSTGRES_PASSWORD` 相同。不要用样例值启动生产环境。
3. 用 `mise run deploy:check` 检查配置，再运行 `mise run deploy:network:create` 创建应用入口网络和 Portainer 专用代理网络。后者为 Docker internal 网络，仅由 Caddy 与 Portainer 共享。保留 `production.env` 的服务器外安全副本；后续备份任务会覆盖入口状态及 Portainer 状态。
4. `mise run deploy:portainer:up` 启动面板。在本机通过 `ssh -N -L 19443:127.0.0.1:9443 用户@服务器` 建立隧道，再访问本机的 `https://127.0.0.1:19443` 完成首次初始化；这里的 `127.0.0.1:19443` 经 SSH 转发到服务器的 `127.0.0.1:9443`，无须对公网开放 9443 或 19443。面板证书初始为自签名。Caddy 启动并签发证书后，日常访问 `https://portainer.fuxiaochen.com/`；SSH 隧道保留作恢复入口。若首次页面要求 setup token，在服务器受保护的终端运行 `mise exec -- docker compose -f deploy/compose.portainer.yaml logs --tail 100 portainer`，读取容器日志中的 token 并直接输入面板；不要将 token 写入文档或聊天。初始化窗口过期时重启 Portainer，再从新日志中读取 token。
5. 在 Portainer 创建应用 Stack。首次 API 启动会自动执行版本化迁移；确认 API 与 PostgreSQL 健康后，运行 `mise run deploy:migrate` 再次核对独立迁移入口。使用 `ADMIN_USERNAME=实际用户名 mise run deploy:admin:init` 在交互终端输入两次密码；密码不通过参数或环境文件传递。再创建入口 Stack，运行 `mise run deploy:status`，并从浏览器验证 `https://管理子域/` 和 `https://管理子域/api/health/ready`。

Portainer 中先建立仅管理员可管理的公开 Git Source，仓库为 `https://github.com/aifuxi/mindfolio.git`，关闭 Source 自动轮询。再分别建立名为 `mindfolio-app`、`mindfolio-edge` 的 Stack，选择 Repository 和该 Source，引用 `refs/heads/master`，Compose 路径分别为 `deploy/compose.app.yaml`、`deploy/compose.edge.yaml`，关闭自动 GitOps 更新。将受保护的 `production.env` 中对应变量导入 Stack 环境变量，保持与服务器恢复文件一致。入口 Caddyfile 是宿主机绝对路径挂载；Portainer CE 不会仅凭 Git 仓库中的相对文件自动把它放到宿主机。首次发布直接由 Portainer 创建 Stack；`deploy:db:up`、`deploy:app:up`、`deploy:edge:up` 留作面板外恢复，不能先从 CLI 启动再重复创建 Stack。Portainer 的 Git Stack 与环境变量用法见[官方文档](https://docs.portainer.io/user/docker/stacks/add)。

## 发布与迁移

1. 在通过 `mise run ci` 的提交上读取 GitHub Actions `publish` 摘要，记录完整提交号及 API、管理端、入口镜像 digest。镜像 tag 用于追溯，部署变量使用 `name@sha256:...`。先核对数据库备份有效恢复点。
2. 对照新旧镜像所含迁移脚本评估兼容性。当前最新迁移为 `202609270011_weekly_review.sql`；仅已部署的同一 API 版本及其同一迁移集合经过验证，**未承诺更早镜像可读取迁移后的数据库**。如果新迁移可能破坏旧程序，先安排维护窗口，停止旧 API 与管理端，再迁移；若是经过验证的向后兼容迁移，可保持服务运行。镜像回退不等于数据库回滚，需从备份恢复才能回到旧模式及数据状态。
3. 在 `production.env` 中把 `API_IMAGE`、`ADMIN_IMAGE` 改为同一提交的新 digest，运行 `mise run deploy:migrate`。这会用新镜像中的独立 `migrate` 程序处理现有数据库；失败时停止发布并核对数据库状态。API 启动时也会执行幂等迁移，但这不替代发布前的显式步骤。
4. 在 Portainer 的 `mindfolio-app` Stack 更新 `API_IMAGE`、`ADMIN_IMAGE` 环境变量为 `production.env` 中的 digest，手动重新部署。确认健康状态、登录、Cookie、CSRF 和日志，并核对面板变量与服务器文件一致。如果入口镜像或 Compose 网络变化，单独更新 `mindfolio-edge` Stack；只有站点文件变化时运行 `mise run deploy:edge:config:sync`，再运行 `mise run deploy:edge:reload` 验证并重载。入口容器重建会造成短暂中断。

API、PostgreSQL 没有公网 `ports`，PostgreSQL 只连应用内网，API 只连应用内网；管理端桥接应用内网和入口网络。入口连接应用入口网络与 Portainer 专用代理网络；Portainer 仅在专用网络内提供 HTTP 9000，9443 保持回环绑定。每个服务有 Docker 日志轮转、CPU/内存上限与停止宽限期。2 核 4 GB 的初始限额需要在任务 04 按实际峰值复核，避免数据库或 API 被 OOM 杀死。查看日志可运行 `mise exec -- docker compose --env-file /srv/mindfolio/config/production.env -f deploy/compose.app.yaml logs --tail 100 api`；不要运行会打印秘密的 `docker compose config`，只用 `config --quiet`。

## 新增域名映射

每个域名在 `deploy/sites/` 下使用独立 `.caddy` 文件，Caddy 统一处理公开 80/443 和 HTTPS。先配置指向服务器的 DNS 记录，确认目标服务与入口之间有合适的 Docker 网络，再新增站点文件并验证。目标服务端口仅供代理网络访问，不发布到公网；运行 `mise run deploy:edge:config:sync` 同步宿主机文件后，再运行 `mise run deploy:edge:reload`。若要增减入口容器所连网络，还需单独更新 `mindfolio-edge` Stack。Portainer 公网域名由自身账号密码控制，管理者选择不加来源 IP 限制；管理员应维护高强度凭据与 Portainer 安全更新。

不希望把内网上游 IP 写入仓库时，可直接在服务器 `/srv/mindfolio/edge/caddy/sites/` 下创建 `local-*.caddy` 站点文件，再运行 `mise run deploy:edge:reload`。`deploy:edge:config:sync` 会保留这些文件；仓库中的 `deploy/sites/` 不得使用 `local-` 文件名前缀。入口状态备份会保存整个 Caddy 配置目录，恢复时核对这些服务器私有文件。

## 面板或入口故障

SSH 是独立恢复入口。Portainer 域名或 Caddy 不可用时，使用上述 SSH 隧道直连 `127.0.0.1:9443`；面板容器故障时运行 `mise run deploy:portainer:up`。入口故障时先检查 `/srv/mindfolio/edge/caddy/Caddyfile`、AliDNS 权限、DNS TXT 传播、`mindfolio_edge_data` 卷及 `mise run deploy:status`，再用 `mise run deploy:edge:reload` 或 `mise run deploy:edge:up` 恢复。

面板无法操作 Stack 时，在原 `/srv/mindfolio/repo`、原 `production.env` 与原 Caddy 挂载路径下运行 `mise run deploy:network:create`、`mise run deploy:db:up`、`mise run deploy:migrate`、`mise run deploy:app:up`、`mise run deploy:edge:up`。Compose 文件中的 project 名、服务名和卷名保持不变，避免生成空数据库或新的证书状态。恢复面板后核对 Stack 记录与 Docker 现状；不要直接删除卷或重建同名 Stack。Portainer 数据、Caddy `/data` 与 `/config`、PostgreSQL 数据的服务器外备份步骤见[离站备份与恢复点](./backup.md)，实际恢复演练由任务 05 完成。
