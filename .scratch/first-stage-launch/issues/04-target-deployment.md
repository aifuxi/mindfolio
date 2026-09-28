# 在目标环境验证手动发布与运行

Status: done

## 目标

在实际目标服务器上证明第一阶段镜像和部署配置可运行、可更新，并保留故障恢复入口。本次正式目标经管理者确认运行 Debian 12。

依据：[本批规格](../spec.md)、[部署 ADR](../../../docs/adr/0005-caddy-portainer-deployment.md)。

## 实施范围

1. 核对服务器 CPU 架构、内存、磁盘、DNS、端口、GHCR 权限、实际管理域名和秘密配置；通过 `mise` 任务完成 Docker、网络、卷和必要目录的准备与诊断。
2. 启动 Portainer、入口 Caddy Stack 与应用 Stack，拉取任务 01 发布的 digest，执行版本对应的数据库迁移和唯一管理者初始化。记录实际运行的提交、镜像 digest 与迁移版本。
3. 在目标环境验证 HTTPS、同源 API、`AUTH_ORIGIN`、生产 Cookie、CSRF、未登录拒绝、数据库与 API 不暴露公网端口，以及 SSH 隧道访问 Portainer。
4. 演练一次由管理者在 Portainer 手动选择版本的更新，检查迁移兼容说明、服务健康、日志轮转、请求标识、优雅退出和目标机器的 CPU、内存、磁盘峰值。
5. 验证任务 03 的真实备份作业和服务器外有效恢复点；记录异常时的 systemd 失败状态与结构化日志。

## 验收条件

- 目标服务器能从 GHCR 拉取指定 digest 并完成首次部署与手动更新；管理端关键私人流程可用。
- 入口、认证、内部网络和恢复入口符合配置；Portainer 停止或 Caddy 故障时仍可通过 SSH 路径管理与恢复。
- 有实际资源数据、健康状态、备份时间和异常状态的验收记录；不以本地模拟代替目标环境检查。

## 依赖与交接

- 依赖任务 01 至 03。目标服务器、DNS、GHCR 和正式 OSS Bucket 可用时才能完成真实验收。
- 发布前先保留目标环境的恢复点；本任务只完成部署验证，正式开放入口以任务 05 的恢复演练结果为准。

## 实施记录

- 2026-09-28：管理者确认以现有 Debian 12 服务器作为正式目标。只读预检显示 `x86_64`、2 核、约 3.5 GiB 内存、根分区约 53 GiB 可用；系统时间已同步，Docker 29.8.1 与 Compose v5.5.1 可用。接入前没有 Mindfolio 目录、容器、网络、卷或相关监听端口。
- 已将备份实现提交 `08d723b49d0d1713880cfb3ea4d1ae83fb78978f` 单独推送到 `master`；目标机现已检出 `d505c17`。安装 mise 2026.6.14，`mise run setup` 与首次使用的镜像提交 CI 发布通过，建立受保护的配置目录和 Caddyfile 挂载路径。
- 首次从 Docker Hub 拉取 Portainer 时 TLS 握手失败；管理者调整目标机网络后重试成功。已从原定固定 digest 启动 Portainer，9443 仅绑定 `127.0.0.1`，容器健康；PostgreSQL 固定 digest 也已拉取。临时增加的 GHCR 第三方镜像同步已用单独的中文 Conventional Commit 撤销，不作为正式部署配置。
- 管理者确认正式管理域名为 `admin.fuxiaochen.com`、正式备份 Bucket 为上海地域的 `mindfolio-backup`，两套专用 RAM 凭据已创建。服务器解析该管理域名到本机公网地址，Bucket HTTPS 入口对匿名请求返回 403；这还不能代替权限、版本控制和实际读回验收。
- 已生成权限为 `0600` 的 `production.env`、`backup.env` 和随机 restic 密码文件，填入已确认的非秘密参数与提交 `08d723b` 的镜像 digest。管理者随后在服务器侧填入两套 AccessKey，并确认服务器外安全副本已保存。
- 管理者已在服务器侧填入彼此独立的 AliDNS 与 OSS RAM 凭据，`mise run deploy:check` 通过。正式 Bucket 的 `mise run backup:init` 在检查 `restic/config` 时收到 OSS `Access Denied`；需要核对备份 RAM 对 Bucket 的列举权限和 `restic` 前缀的对象读写权限，修复前不启动应用。
- Portainer 管理员初始化已由管理者完成，本机经 SSH 隧道访问面板的管理员状态接口返回 HTTP 204。9443 仍仅监听服务器回环地址，未对公网开放。
- 管理者调整正式 Bucket 的 RAM 权限后，加载服务器 `backup.env` 执行 `mise run backup:init` 成功。在独立的 `restic/validation` 前缀初始化测试仓库，上传 `/etc/debian_version` 并从 OSS 读回逐字节比对通过。此测试快照不计入正式数据库恢复点。Portainer 已建立仅管理员可管理的公开 Git 源 `mindfolio`，连接到 `https://github.com/aifuxi/mindfolio.git`，连接检查通过且未启用自动轮询。
- OSS 控制台显示正式 Bucket 的版本控制尚未开启。管理者了解历史版本会产生存储费用、开启后只能暂停不能关闭，并明确选择暂不启用；该项不满足任务 03 既定保护要求，后续验收需单独记录。管理者确认 `production.env`、`backup.env` 和 restic 密码已保存服务器外安全副本。
- Portainer 以关闭自动轮询的 Git Source 建立 `mindfolio-app` 和 `mindfolio-edge` 两个 Stack，首次部署使用 `08d723b` 的不可变镜像 digest。PostgreSQL、API、管理端和入口容器均健康；数据库迁移完成，最新版本 `202609270011_weekly_review.sql`。唯一管理者 `mindfolio_with_chen` 由管理者在自己的交互终端初始化，未在聊天中传递密码。
- AliDNS RAM 策略的域名资源 ARN 曾包含未替换的 `ACCOUNT_ID`。经管理者确认后，仅将该占位符改成实际账号 ID；Caddy 的 DNS challenge 随后通过。因上一轮 TXT 缓存仍在，默认 2 分钟传播等待导致正式签发超时；将站点传播等待改为 15 分钟、验证配置并重载后，2026-09-28 14:58:21 CST 成功取得正式证书。公网直连 `47.100.90.112` 验证 HTTPS 主页和 `/api/health/ready` 均为 200，TLS 校验通过；HTTP 返回 308 跳转 HTTPS，未登录访问 `/api/projects` 返回 401 并带请求标识。
- 实际备份作业在正式 OSS 仓库生成并读回校验数据库快照。15:00 CST 的自动作业首次尝试恰逢 Portainer 重建数据库容器而失败，内置重试在 15:01:04 CST 成功，快照 `1240721838740ca7c01c01acdb846292ba6362ee823f37c1dda8cbd62e4e979e`；独立 `backup:verify` 再次通过。入口状态作业在 15:01:54 CST 成功，快照 `11639d5949f1611af0738792e24411fc2d4345c69826d3ada63518460e615242`，独立读回校验通过。四个备份 timer 均已启用；当前无失败的 systemd unit。
- Portainer 手动更新应用 Stack 到 `f5f289e` 发布的 API 和管理端 digest；显式迁移任务通过，三个应用容器重建后均健康。`production.env` 已同步对应 digest。目标机只有 80/443 对公网监听，Portainer 9443 仅绑定 `127.0.0.1`；数据库 5432 与 API 3001 未发布。五个容器使用 `json-file` 日志轮转，上限 10 MiB × 3。目标机根分区约 51 GiB 可用，内存约 2.7 GiB 可用；容器观测内存均远低于各自限额。
- 入口状态备份短暂停止入口和面板后成功恢复；Caddy 日志记录收到 `SIGTERM` 后以退出码 0 完成优雅关闭，随后加载原有证书重新提供服务。正式证书由 Let's Encrypt 签发，主题为 `admin.fuxiaochen.com`，有效期至 2026-12-27 13:59:48 CST。应用内网 `mindfolio-app_app` 为 Docker internal 网络；只有入口容器加入可对外网络。未经登录的 API 请求返回结构化 `request_id`。
- Portainer 设置检查的工具输出意外包含数据库密码。管理者在 Portainer 保存全新密码但未立即重建，随后服务器更新数据库角色与 `production.env`，再由 Portainer 手动重新部署；使用新连接串的显式迁移、三个应用容器健康检查、HTTPS 主页和 API 就绪检查均通过。轮换后的数据库恢复点为 `7b2d614dfeb1cc0f8718c552ef7853b6e649b5c7af836da23988b72f09ed6b10`（2026-09-28 15:10:41 CST），入口状态恢复点为 `b6ecab0ee5e21c145611f1fee55ee65947e45076e3f4f9299be1c06cdea23de9`（15:10:48 CST）；两个快照再次执行 `backup:verify` 均通过。临时新密码文件已删除。
- 管理者确认服务器外密码管理器已更新为当前的 `production.env`。配置修复提交 `d505c17` 的 GitHub Actions「工程检查」通过。轮换后资源采样：目标机根分区 59 GiB 中已用 6.1 GiB、可用 51 GiB；内存 3.5 GiB 中可用约 2.7 GiB。当前五个容器 CPU 均不超过 0.7%，容器启动以来的 cgroup 内存峰值依次为 PostgreSQL 35.3 MiB、API 12.1 MiB、管理端 25.6 MiB、入口 24.7 MiB、Portainer 23.5 MiB；这些是短时采样和容器重启后的内存峰值，不代表持续负载测试。
- 管理者已在正式域名登录并确认页面正常显示。正式环境的未登录私人 API 返回 401；认证 Cookie 的 `Secure`、`HttpOnly`、`SameSite=Strict` 属性，以及缺失或错误 CSRF、错误 Origin 拒绝，在本次镜像对应的真实数据库自动化测试中通过。此次未提取浏览器 Cookie，也未在正式数据中创建测试记录。部署后的浏览器写操作没有单独执行，任务 05 的恢复演练仍未开始。
- 15:18 CST 对一次真实数据库备份每 0.2 秒采样，24 次样本中目标机 CPU 最高占用 68.4%、内存最高已用 880.7 MiB、根分区最高已用 6.01 GiB；作业成功并创建已校验数据库快照 `1de44806d918b8f8134ef08a34a1000b591410ad34ee57b60da1e2740c47a2fc`。这些是该次作业采样范围内的峰值，不代表长期业务流量峰值。

## 验收结论

- 首次部署、Portainer 手动更新、迁移、正式证书、公开入口、认证读取、内部端口隔离、SSH 面板恢复入口、systemd 调度和正式 OSS 恢复点已在 Debian 12 目标机验证。入口 Caddyfile 已通过 `deploy:edge:reload` 在原容器内生效，入口镜像本身未变。
- 正式 Bucket 版本控制按管理者决定暂不启用，因此任务 03 关于历史版本保护的要求仍未满足。本任务记录该例外，不将其写成已经启用。任务 05 的隔离恢复演练仍是后续正式开放验收的独立条件。
