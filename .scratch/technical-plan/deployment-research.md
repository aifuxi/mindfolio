# Caddy 与 Portainer 部署调研

日期：2026-09-27

状态：部署管理方案已确认，决策见 [ADR 0005](../../docs/adr/0005-caddy-portainer-deployment.md)；尚未连接服务器或执行部署验证。

## 已知环境

自有 Ubuntu 服务器或 VPS，预计 2 核 CPU、4 GB 内存，环境从零搭建。已选择 Docker、Caddy 和 Portainer；应用采用 Axum + Tokio + SQLx + PostgreSQL，前端采用 Vue 3 + TypeScript。

公开网站与管理端采用分子域名访问：主域名用于公开网站，`admin` 子域名用于管理端。`example.com` 与 `admin.example.com` 仅是地址示例，实际域名尚未提供；Portainer 的日常域名入口另行配置。

## 已确认方案与取舍

Caddy 容器化，通过 Portainer 的独立 Stack 管理；Portainer 本身用服务器上的 Compose 配置启动。Compose 文件、Caddyfile、所需变量和恢复说明保留在面板之外，使日常管理方便，故障时也可通过 SSH 恢复。

| 方式 | 适用性与取舍 |
|---|---|
| Caddy 作为 Portainer 独立 Stack | 已采用；通过面板部署、查看日志和管理容器，保留 SSH 恢复入口 |
| Caddy 由服务器 Compose 管理，Portainer 只查看 | 可选；配置直接位于服务器，但 Portainer 对外部创建资源的管理能力有限 |
| Caddy 安装在宿主机，由 systemd 管理 | 适合明确要求入口独立于 Docker 生命周期的情况；会增加一套服务管理方式 |

Portainer 支持通过 Compose 格式的文件创建 Stack；在它之外创建的 Docker 资源不等于自动获得完整的 Stack 配置管理能力。[创建 Stack](https://docs.portainer.io/user/docker/stacks/add)、[外部资源管理](https://docs.portainer.io/advanced/access-control)

## 运行与管理的边界

Portainer 不在业务请求路径中。它停机时，Docker 中的既有工作负载继续运行；容器启动与退出后的重启由 Docker 策略处理。[Portainer 运行模型](https://learn.portainer.io/module-12)、[Docker 重启策略](https://docs.docker.com/engine/containers/start-containers-automatically/)

以下是据此得出的部署判断：

| 故障或操作 | 影响 |
|---|---|
| Portainer 停机或升级 | 面板暂不可用，正常运行的 Caddy 与应用继续服务 |
| Caddy 不可用 | 经它代理的网站不可访问；若 Portainer 域名也经过 Caddy，该入口同时不可访问 |
| 只更新应用 Stack | 独立的 Caddy Stack 可以继续运行，应用更新期间可能短暂不可用 |
| Docker 或 VPS 整体故障 | 同机容器仍有共同故障边界，拆 Stack 不等于高可用 |

保留 SSH 作为恢复入口。Portainer 的 9443 端口绑定到服务器回环地址，必要时通过 SSH 隧道直连；即使日常通过 Caddy 域名访问 Portainer，也可以绕过 Caddy 恢复。Portainer 官方支持独立启动并持久化自己的数据，默认 HTTPS 管理端口为 9443。[Portainer Linux 安装](https://docs.portainer.io/start/install-ce/server/docker/linux)

## 已确认的服务组织

| 单元 | 内容 | 管理方式 |
|---|---|---|
| Portainer | 管理面板及其数据卷 | 服务器 Compose 启动与恢复，首次安装经 SSH 完成 |
| 入口 Stack | Caddy、配置目录、证书状态卷 | Portainer 日常管理，独立于应用发布 |
| Mindfolio Stack | API、PostgreSQL，以及按最终前端方案确定的页面服务 | Portainer 管理应用发布 |

Caddy 与需要被代理的服务加入共享 external network，以服务名访问；PostgreSQL 只接入应用内部网络。Docker 支持跨 Compose 项目共享网络，外部网络需预先创建。[Compose 网络](https://docs.docker.com/compose/how-tos/networking/)

## Caddy 配置与状态

- 使用官方镜像，实施时固定经过验证的版本。
- `/data` 必须持久化并备份，其中包含证书、私钥等状态；同时持久化 `/config`。
- 服务器固定目录保存 Caddyfile，将整个目录挂载到 `/etc/caddy`，避免单文件挂载在编辑器替换文件后仍指向旧文件。
- 仅修改 Caddyfile 时，验证后执行 `caddy reload`；镜像、挂载或端口等容器配置变化时需要重建容器，单实例重建应预期有短暂中断。

这些目录和重载方式来自 [Caddy 官方镜像说明](https://hub.docker.com/_/caddy)。证书申请与续期由 Caddy 自动完成，常规公网域名需要正确的 DNS 和挑战端口访问条件；与 Portainer 是否在线无关。[Caddy 自动 HTTPS](https://caddyserver.com/docs/automatic-https)

## 配置来源与面板外恢复

Compose 与 Caddyfile 纳入版本管理，服务器保留对应部署文件；密钥和实际环境变量另行保存。Git Stack 的 Compose 在 Git 中修改后重新部署，Web Editor Stack 则在面板编辑，实施时确定一种日常配置来源，避免两边独立修改。[编辑 Stack](https://docs.portainer.io/user/docker/stacks/edit)

Portainer 从 Git 自动准备相对路径挂载文件的功能属于 Business Edition。首版可以使用服务器固定目录的绝对路径挂载 Caddyfile，不依赖该功能；单纯拉取 Compose 不能假定同仓库中的配置文件已经出现在宿主机挂载路径。[相对路径挂载说明](https://docs.portainer.io/user/docker/stacks/add#relative-path-volumes)

通过 Compose CLI 恢复时，必须保留原 project 名、服务名、卷与网络名称、变量和挂载路径，避免创建另一套资源；恢复后再核对 Portainer 中的记录。Portainer 的数据备份、Caddy 状态备份和 PostgreSQL 业务备份各有职责，不能互相替代。[Compose up](https://docs.docker.com/reference/cli/docker/compose/up/)、[Portainer 备份范围](https://learn.portainer.io/module-12)

## 2 核 4 GB 的实施建议

按当前单管理者、首版低访问量的使用预期，可将该配置作为起步方案；这是容量规划判断，尚无本项目实测数据。建议在开发机或 CI 构建镜像，服务器负责拉取与运行；依据实际内存和 CPU 指标设置容器限额，为系统、数据库与短时峰值留余量。Docker 默认不限制容器资源，支持显式设置内存与 CPU 约束。[Docker 资源约束](https://docs.docker.com/engine/containers/resource_constraints/)
