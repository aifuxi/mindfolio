# 第一阶段离站备份与恢复点

本方案使用 restic 0.19.1 将 PostgreSQL 自定义格式归档和入口状态加密后存入独立私有 S3 兼容 OSS Bucket。加密在客户端完成。数据库每 30 分钟备份一次，数据库恢复点以 `pg_dump` 开始前的 UTC 时间计时；目标为 RPO 不超过 1 小时。正式 Bucket 与 RAM 授权在接入时填写，未通过真实接入前不能把本地测试视作上线验收。管理者明确不配置外部告警渠道，异常仅记录到作业日志和 systemd 失败状态，需人工巡检。

## 服务器准备

1. 在 `/srv/mindfolio/repo` 安装项目锁定的 mise 工具，执行 `mise trust` 和 `mise run setup`。备份进程使用与应用部署相同的 Docker 权限。系统需提供 `tar`、`systemd`、Docker Compose。校准系统时钟。
2. 创建与业务文件分离的私有备份 Bucket，仅允许备份身份访问该 Bucket 的备份前缀。备份与定时清理身份需要列举、读取、上传和删除该前缀中的对象，分片上传时还可能需要列举与中止分片的权限；不要授予整个账号的 OSS 管理权限。开启 Bucket 版本保护，并为恢复旧版本配置独立管理身份。
3. 把 `deploy/backup.env.example` 保存为 `/srv/mindfolio/config/backup.env`，填入真实的 S3 兼容 endpoint、Bucket、区域和专用 RAM 凭据，权限设为 `0600`。例如杭州地域使用 `RESTIC_REPOSITORY=s3:https://s3.oss-cn-hangzhou.aliyuncs.com/私有Bucket名/restic` 和 `AWS_DEFAULT_REGION=cn-hangzhou`。脚本会为 OSS 显式设置 restic 的 `s3.bucket-lookup=dns`，以使用 OSS 要求的虚拟主机访问方式；先在真实 Bucket 试运行。
4. 生成高熵随机 restic 密码并保存到 `RESTIC_PASSWORD_FILE` 指定的 `0600` 文件；把密码和备份 Bucket 的恢复凭据分别保存到服务器外密码管理器。
5. 在服务器外密码管理器另存数据库角色、`production.env`、镜像 digest、DNS/OSS RAM 权限说明和恢复凭据。`production.env` 也包含在加密入口状态备份中，但 restic 密码和 OSS 恢复凭据必须独立保存，不能只放在该仓库内部。

真实 `backup.env`、restic 密码及临时明文归档不进入 Git、镜像或普通日志。数据库归档与入口归档暂存于 `/run/mindfolio-backup`，systemd 在作业结束后移除目录；手动执行时设置 `BACKUP_TMP_DIR` 为权限 `0700` 的 tmpfs 目录。服务器同时应限制 root、Docker 与 Portainer 管理权限。

## 首次接入和调度

在受信任的 root 交互终端加载配置，再通过 mise 执行命令。配置文件由管理员编写，按 shell 环境文件语法保存；包含特殊字符的值须正确引用。

```sh
set -a
. /srv/mindfolio/config/backup.env
set +a
export BACKUP_TMP_DIR=/run/mindfolio-backup
mise run backup:init
mise run backup:run
mise run backup:list
```

`backup:init` 只运行一次；如果仓库已存在，不要重新初始化。确认 `backup:list` 显示 `mindfolio-db`、快照时间、完整 ID 和 SHA-256 后，把 `deploy/systemd/mindfolio-backup*.service` 与 `*.timer` 复制到 `/etc/systemd/system/`，运行 `systemctl daemon-reload`、`systemctl enable --now mindfolio-backup.timer mindfolio-backup-age.timer mindfolio-backup-state.timer mindfolio-backup-prune.timer`。用 `systemctl list-timers 'mindfolio-backup*'` 检查调度，并立即手动运行 `systemctl start mindfolio-backup-state.service`，确认入口状态的第一份恢复点。以上服务使用 root 运行以访问 Docker；正常输出只含事件、时间、摘要和快照 ID。

如果目标服务器尚未部署，可先在本机用真实 Bucket 的独立测试前缀初始化 restic 仓库，上传并读回一个无敏感信息的测试文件，以确认 Endpoint、地域和 RAM 授权。此步骤只验证 OSS 接入，不计作生产数据库与入口状态备份验收。

`backup:run` 对导出、上传或读回校验失败最多尝试三次，间隔 1 分钟和 2 分钟。每次备份先执行一致性 `pg_dump -Fc`，restic 加密上传，随后通过 `restic dump` 从服务器外读回全部内容并比对 SHA-256。只有读回成功后才加 `mindfolio-verified` 标签；失败上传或读回失败的快照不进入有效恢复点列表。每 10 分钟的 `backup:age` 以最新有效数据库快照时间计算年龄：50 分钟记录 `warning`，60 分钟及以上记录 `critical`；两种情况均以非零状态退出。备份作业三次失败后也以非零状态退出。值班人员需定期查看 `systemctl --failed 'mindfolio-backup*'` 和 `journalctl -u mindfolio-backup.service -u mindfolio-backup-age.service -u mindfolio-backup-state.service -u mindfolio-backup-prune.service`；系统不会主动推送通知。

`backup:state` 每天执行一次。它分别短暂停止 Portainer 和入口 Caddy 容器，复制 Portainer `/data`、Caddy `/data` 和 `/config`，然后启动服务；再将 `production.env`、宿主机 Caddy 配置目录（`Caddyfile` 与 `sites/*.caddy`）、仓库提交号与快照时间打入 tar 归档。该任务会造成短暂的面板与 HTTPS 入口中断，应在目标环境观察耗时并选择适合的时段。恢复点同样经加密上传、完整读回和 SHA-256 校验。若容器不存在或复制失败，任务失败且不会产生有效入口恢复点。

## 列举、下载与校验

在新机器上安装相同的锁定工具版本，并从服务器外密码管理器取得 restic 密码、OSS 恢复身份和 Bucket 地址，配置同名环境变量。运行 `mise run backup:list` 列出已验证的数据库和入口状态快照。选定完整 ID 后执行：

```sh
BACKUP_SNAPSHOT_ID=完整快照ID mise run backup:verify
mise exec -- restic -o s3.bucket-lookup=dns -o "s3.region=$AWS_DEFAULT_REGION" snapshots --json 完整快照ID
mise exec -- restic -o s3.bucket-lookup=dns -o "s3.region=$AWS_DEFAULT_REGION" ls 完整快照ID
mise exec -- restic -o s3.bucket-lookup=dns -o "s3.region=$AWS_DEFAULT_REGION" dump 完整快照ID /快照中显示的完整路径 > /受保护的临时目录/mindfolio.dump
sha256sum /受保护的临时目录/mindfolio.dump
```

入口状态的文件名为 `mindfolio-state.tar`。下载后与 `backup:list` 所列 SHA-256 比对，再用 `tar -tf` 检查内容，确认 `manifest.json` 的提交号和快照时间；解包到受保护的临时目录。数据库归档用 `pg_restore --list` 检查自定义格式，隔离环境还原由任务 05 完成。临时目录位于加密磁盘或 tmpfs，权限为 `0700`，用完清理。`restic dump` 同时解密并校验仓库数据，外层 SHA-256 证明读回字节与写入时一致。手动 JSON 导出不能替代数据库归档。

恢复入口时沿用 `docs/deployment.md` 的 project、卷和网络名称。将入口 tar 中的 `caddy/`、`edge/data`、`edge/config`、`portainer/data` 恢复到对应宿主路径或原名 Docker 卷，并核对 `manifest.json` 的提交与镜像 digest；再启动 Portainer 和入口。`production.env` 从服务器外密码管理器核对后恢复。凭据若已轮换，使用新凭据并重新检查 DNS challenge 与 HTTPS。入口状态恢复及 PostgreSQL 实际还原必须在任务 05 的隔离环境演练，不能仅凭下载成功认定 RTO 达标。

## 保留和故障处理

`mise run backup:prune` 按快照时间保留最近 24 小时所有有效恢复点；对更早且仍在 30 天内的快照，每个 UTC 日期保留最新一份；另外保留最近一个跨越 24 小时边界的有效点。即使所有快照已超过 30 天，也保留最后一个有效点。没有有效数据库快照时拒绝清理。清理先删除不再保留的快照引用，再由 restic `prune` 回收无引用数据；restic 不会删除仍被保留快照引用的数据。清理失败保留下一次重试机会，并用 `backup:list` 核对剩余快照。未完成校验的快照不作为有效点，也不参与自动清理；故障调查后由管理员核对并手动处理。

故障注入的本地验收使用文件系统 restic 仓库模拟服务器外存储，覆盖导出、上传、读回失败、过旧恢复点和时间边界。它不证明正式 OSS Bucket 的兼容性、RAM 权限、目标 VPS 资源或真实灾难恢复。真实接入时应在备份 Bucket 执行一次 `backup:run`、`backup:state`、`backup:verify`，在独立测试前缀验证 `backup:prune`，并从独立机器读取归档；完成任务 05 恢复演练后记录恢复时间。
