# 使用公开域名访问 Portainer

Status: completed

## 目标

让管理者通过 `https://portainer.fuxiaochen.com/` 访问正式 Portainer，并使入口配置支持后续按域名增加独立站点文件。管理者确认该域名对所有公网来源开放，由 Portainer 账号控制访问。

## 实施范围

1. 将 Portainer 与 Caddy 接入专用 Docker 代理网络，9000 仅在容器网络提供 HTTP，继续将 9443 绑定服务器回环地址作为 SSH 恢复入口。
2. 将 Caddyfile 拆为共享 AliDNS 配置和每域名独立站点文件，增加 Portainer 代理路由。
3. 更新配置同步、隔离部署验证、入口状态备份与恢复文档。
4. 在目标服务器验证证书、域名访问、现有管理端可用性、端口边界及离站入口状态恢复点。

## 验收条件

- `https://portainer.fuxiaochen.com/` 的 HTTPS 证书有效，Portainer 登录页面可用。
- `https://admin.fuxiaochen.com/` 继续正常工作；9000 和 9443 不对公网监听，SSH 隧道恢复入口仍可用。
- 入口状态恢复点包含全部站点文件，并通过 OSS 读回校验。

## 讨论

- 管理者先要求只分析，随后确认按公开域名和可扩展站点文件方案实施；不配置来源 IP 白名单或 VPN 限制。
- 管理者在 Portainer Stack 重新部署前确认公开访问变更。

## 实施与验收

- 2026-09-28 将代码提交 `4c02813` 同步到正式服务器，并在 Portainer 中重新部署 `mindfolio-edge` Stack；Caddy 校验和重载通过。
- `https://portainer.fuxiaochen.com/` 返回 200，Chrome 显示 Portainer 登录页；TLS 证书由 Let's Encrypt 签发，`openssl s_client` 校验结果为 `Verify return code: 0 (ok)`。
- `https://admin.fuxiaochen.com/api/health/ready` 返回 200 和 `{"status":"ok"}`。
- 专用 `mindfolio_portainer_proxy` 网络为 internal，仅连接 Caddy 与 Portainer；9000 无宿主机监听，9443 仅监听 `127.0.0.1`，80/443 对公网监听。
- 变更前入口状态快照 `e4561be9a60225223682068f9c3a487b55238cafb55d63a64d78e8f2fef8bc33` 已通过 OSS 读回校验。变更后快照 `c7b05adcae4456e6b17443cb757e6a8dc278faeb6541eeb67e83578e0246bc5d` 已通过自动读回和 `backup:verify` 独立校验；备份脚本将整个 Caddy 目录（含各站点文件）纳入归档。
- 本地通过 `mise run deploy:verify`、`mise run image:verify`、`mise run check` 和备份脚本测试。签发时服务器 Mihomo DNS 的旧结果缓存曾延迟传播检查；缓存到期后 TXT 记录可见，Caddy 成功签发证书，未更改代理配置。
