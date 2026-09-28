# 使用 Portainer 管理独立的 Caddy Stack

在自有 Ubuntu 服务器的 Docker 环境中，Caddy 容器化并作为独立 Stack 交给 Portainer 管理，Mindfolio 应用使用另一套 Stack，使入口服务可独立于应用发布维护。Portainer 本身由服务器上的 Compose 配置启动与恢复，Compose、Caddyfile 和所需环境配置保留在面板之外，Caddy 状态与应用数据持久化并分别备份。保留 SSH，并通过绑定回环地址的 Portainer 9443 端口提供 SSH 隧道访问，确保 Caddy 或面板故障时仍有恢复路径；具体依据与要求见 [部署调研](../../.scratch/technical-plan/deployment-research.md)。

实际域名的权威 DNS 托管在阿里云，因此入口 Caddy 使用加入 `caddy-dns/alidns` 的自建固定版本镜像，通过 DNS challenge 签发与续期证书。这样可以按用户要求使用 DNS 验证，但入口需要独立的最小权限 RAM 凭据、可用的 AliDNS API，以及对 Caddy `/data` 证书状态的持久化和备份；凭据只通过服务器部署环境提供。
