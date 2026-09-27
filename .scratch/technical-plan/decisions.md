# 技术选型确认与讨论记录

状态：确认过程归档。当前实施依据以[技术实施规格](./spec.md)为准；以下保留选择形成时的讨论、建议和参考资料。

## 项目起点

本项目从零建设，没有历史业务数据需要迁移。`/Users/chen/Downloads/rust-blog-rest-api-plan-v1.5.md` 仅作为技术选型参考，不自动继承其中的必选要求、功能范围或交付顺序。

功能范围以 [已确认的功能规格](../product-scope/spec.md) 为准：先交付个人管理，后交付公开网站。

## 已确认的技术方向

| 项目 | 选择 |
|---|---|
| 接口契约 | Rust 请求与响应 DTO → OpenAPI → 自动生成 TypeScript 类型，使用 REST/JSON，不强制采用 Protobuf |
| 数据库与业务主键 | PostgreSQL，使用 `BIGINT IDENTITY` 配合主键约束，不采用雪花 ID |
| 后端框架与运行时 | Axum + Tokio |
| 数据访问 | SQLx |
| 文件存储 | 接入外部 S3 兼容对象存储，预期使用阿里云 OSS；具体区域、Bucket 和 SDK 待接入时确定 |
| 文章图片权限 | 随所属文章的公开状态变化；草稿图片私有，撤回后拒绝通过该文章发起的新的公开图片访问 |
| 后端架构 | 模块化单体，按业务模块组织一个后端应用 |
| 部署环境 | 自己的 Ubuntu 服务器或 VPS，从零搭建，预计 2 核 CPU、4 GB 内存 |
| 容器运行 | Docker |
| HTTPS 与反向代理 | Caddy |
| 服务管理 | Portainer 管理独立的 Caddy Stack 和 Mindfolio Stack；Portainer 本身由服务器 Compose 启动与恢复 |
| 代码与镜像发布 | GitHub 托管代码，GitHub Actions 检查并构建镜像，推送 GHCR；通过 Portainer 手动选择版本部署 |
| 部署恢复 | 配置保存在面板之外，保留 SSH 及绕过 Caddy 的 Portainer 管理入口 |
| 数据恢复目标 | 服务器故障后从备份恢复，最多接受丢失最近 1 小时的数据（RPO ≤ 1 小时），24 小时内恢复使用（RTO ≤ 24 小时） |
| 历史备份保留 | 保留 30 天；最近 24 小时保留较密集的恢复点，更早每天保留一份 |
| 站点访问 | 公开网站使用主域名，管理端使用 `admin` 子域名；例如 `example.com` 与 `admin.example.com`，实际域名待提供 |
| 管理者登录 | 本地账号密码，沿用功能范围中唯一管理者的限制 |
| 密码与会话 | Argon2id 密码哈希；服务端 Session 存储于 PostgreSQL，使用仅管理子域可用的安全 Cookie |
| 账号维护 | 通过服务器命令初始化唯一管理者及重置密码 |
| 前端 | Vue 3 + TypeScript |
| 管理端渲染 | Vite 构建的 Vue SPA，由 Caddy 托管静态产物 |
| 第一阶段文本编辑 | 任务说明与每日记录采用 Markdown 编辑和预览，保存 Markdown 原文 |
| 完成历史保留 | 删除任务或项目后，日常列表移除相关内容，已有任务完成记录继续用于每日汇总与每周回顾 |
| 公开网站渲染 | 第二阶段采用 Nuxt SSR，在请求时读取最新已发布内容，内容变更无需重新构建网站 |
| 工程组织 | 单仓库维护 Rust API、Vite 管理端和 Nuxt 公开端，各自独立构建与部署 |
| 前端依赖与共享代码 | pnpm workspace，共享由 OpenAPI 生成的 TypeScript 接口类型；Rust 依赖与构建使用 Cargo |
| 业务时区 | 固定使用 `Asia/Shanghai` 计算“今日”、习惯打卡、每日记录与每周回顾的日期边界 |
| 组件库 | 使用管理者自行维护的 `@aifuxi/semi-ui-vue`，作为前端组件与主题基础 |

契约选择见 [ADR 0001](../../docs/adr/0001-rust-dto-openapi-typescript.md)，主键选择见 [ADR 0002](../../docs/adr/0002-postgresql-identity-primary-keys.md)。

组件库选择见 [ADR 0003](../../docs/adr/0003-semi-ui-vue.md)。[组件库仓库](https://github.com/aifuxi/semi-ui-vue) 当前要求 Vue 3.5+，提供根入口与逐组件导入；实施时按实际发布包选择并锁定版本，不以源码仓库版本号代替已发布版本。

无旧数据迁移不影响功能规格中要求保留的任务完成历史、习惯打卡历史和每日记录，也不取消工程实施中的数据库结构迁移。

后端技术栈与模块化单体选择见 [ADR 0004](../../docs/adr/0004-rust-modular-monolith.md)。

Caddy 与 Portainer 的管理方式已确认，见 [ADR 0005](../../docs/adr/0005-caddy-portainer-deployment.md)；[部署调研](./deployment-research.md) 保留具体组织方式、恢复要求与依据。

## 已确认的登录实现

唯一管理者使用本地账号密码登录，配套实现已确认，见 [ADR 0006](../../docs/adr/0006-local-account-postgresql-session.md)。

- 使用服务端 Session，存储在现有 PostgreSQL 中；登录成功轮换会话标识，退出撤销当前会话，改密或重置密码撤销全部会话。
- 使用 Argon2id 保存密码哈希；首次创建管理者及忘记密码后的重置通过服务器命令完成，不扩展公开注册或邮件找回密码流程。参数、并发限制和会话有效期在实施时确定。
- 管理端从自身域名的 `/api` 访问 Rust API，Session Cookie 仅作用于管理子域，设置 `Secure`、`HttpOnly`，不设置跨子域的 `Domain`；管理接口由 Rust 逐请求鉴权，并校验 CSRF token 和请求来源。

安全依据：[OWASP 密码存储](https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html)、[会话管理](https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html)、[CSRF 防护](https://cheatsheetseries.owasp.org/cheatsheets/Cross-Site_Request_Forgery_Prevention_Cheat_Sheet.html)。

## 已确认的公开网站渲染方向

公开网站使用 Nuxt SSR，在文章发布、修改或撤回后立即反映最新状态，见 [ADR 0007](../../docs/adr/0007-nuxt-public-ssr.md)。具体实现遵循以下约定：

- “立即生效”以 Rust API 完成数据库事务并返回写入成功为起点，此后发起的公开读取反映最新状态。已经打开的页面刷新后读取新内容；搜索引擎的重新抓取不受应用控制。
- Rust API 负责发布状态过滤与数据读写，Nuxt 只读取公开接口并渲染。草稿及撤回的文章不出现在公开列表中，公开详情请求返回 `404`，不输出正文。
- 首版对依赖可变公开内容的 HTML、API 响应及对应 `404` 使用 `Cache-Control: no-store`，不启用相应路由的预渲染、SWR、ISR 或其他跨请求内容缓存；浏览器中的后续内容访问也重新取数，避免复用旧文章状态。带内容哈希的脚本和样式等静态构建资源可独立缓存。
- Nuxt 渲染服务在第二阶段加入部署；届时验证 2 核、4 GB 主机的实际资源占用，以及公开页面所用 `@aifuxi/semi-ui-vue` 组件的服务端渲染与 hydration 兼容性。

依据：[Nuxt 渲染模式](https://nuxt.com/docs/4.x/guide/concepts/rendering)、[HTTP 缓存规范](https://www.rfc-editor.org/rfc/rfc9111.html#section-5.2.2.5)、[Vue SSR 指南](https://vuejs.org/guide/scaling-up/ssr)。

## 已确认的工程组织

在同一仓库组织 Rust API、Vite 管理端与 Nuxt 公开端，分别构建和部署；前端使用 pnpm workspace 共享由 OpenAPI 生成的 TypeScript 接口类型，见 [ADR 0008](../../docs/adr/0008-monorepo-pnpm-workspace.md)。Rust 依赖与构建使用 Cargo；管理端为 Vite 构建的 Vue SPA，由 Caddy 托管静态产物，Nuxt 公开端在第二阶段实现。具体目录和版本在实施规格中确定，参考 [pnpm workspace 文档](https://pnpm.io/workspaces)。

## 已确认的业务时区

业务日期统一以 `Asia/Shanghai` 为准，不随当前设备时区改变。任务计划日期、截止日期、习惯打卡日期和每日记录日期按 PostgreSQL `DATE` 建模；创建、修改和任务完成等实际事件时间使用 `timestamptz` 表示时间点，按业务时区归入相应日期。服务端使用业务时区计算“今日”和统计区间，前端沿用同一约定，日期字段不通过 UTC 时间戳中转。

## 已确认的对象存储方向

文件接入外部 S3 兼容对象存储，预期服务商为阿里云 OSS，见 [ADR 0009](../../docs/adr/0009-external-object-storage.md)。PostgreSQL 保存文件元信息和对象标识，文件内容保存在对象存储；持久化记录不依赖会过期的签名 URL。

OSS 提供 S3 兼容接口，但不能据此假定所有 S3 SDK 默认行为均可直接使用。接入时根据实际 Bucket 区域验证 Endpoint、virtual-hosted 寻址、签名与传输编码，并使用最终锁定的 Rust SDK 版本验证上传、读取、元信息查询、删除与短时预签名操作。

Rust 接入优先验证 `aws-sdk-s3`，其官方文档支持[自定义 Endpoint](https://docs.aws.amazon.com/sdk-for-rust/latest/dg/endpoints.html)与[预签名 GET／PUT](https://docs.aws.amazon.com/sdk-for-rust/latest/dg/presigned-urls.html)；这仍是待实测候选，不等同于已验证 OSS 兼容。存储 SDK 封装在后端文件存储模块中，业务代码使用项目自己的文件操作接口。

上传方式建议采用后端鉴权后为指定的新对象签发短时上传授权，浏览器直传 OSS，再由后端核验对象并登记；避免上传授权被用于覆盖已有公开图片，长期访问凭证由服务端保管。浏览器跨域配置与上传限制属于接入实现。

文章图片已确认随文章权限变化，配套读取设计如下：

- OSS Bucket 与对象保持私有。公开图片通过带文章上下文的媒体接口访问，由 Rust 校验文章当前发布状态、文章与文件的关联及文件可用状态，再流式读取 OSS 并返回；草稿图片预览只对已登录的管理者开放。
- 媒体接口不向访客发放能够绕过文章状态校验的 OSS 下载直链或预签名 GET URL。图片读取经过 VPS，会增加其出口带宽需求，实施时验证实际负载。
- 图片响应使用 `Cache-Control: no-store`，Caddy 及后续可能引入的 CDN 不缓存此媒体路由；包括 HEAD、条件请求和 Range 请求在内，每次请求均先进行权限校验。
- 撤回事务提交后才开始权限校验的公开请求必须被拒绝；此前已通过校验的在途传输可完成，访客已下载的内容无法收回。若同一文件被多篇文章使用，撤回文章对应的访问入口失效，其他仍公开文章的合法入口继续有效。

依据：[OSS 的 S3 兼容范围](https://www.alibabacloud.com/help/en/oss/developer-reference/compatibility-with-amazon-s3)、[使用 AWS SDK 访问 OSS](https://help.aliyun.com/zh/oss/developer-reference/use-aws-sdks-to-access-oss)、[预签名上传](https://www.alibabacloud.com/help/en/oss/user-guide/upload-files-using-presigned-urls)。目前仅完成文档核对，尚未连接实际 Bucket 验证。

## 已确认的数据恢复目标与备份建议

服务器故障后从备份恢复，允许丢失的数据窗口最多为 1 小时（RPO ≤ 1 小时），服务在 24 小时内恢复使用（RTO ≤ 24 小时）。历史备份保留 30 天：最近 24 小时保留较密集的恢复点，更早每天保留一份；具体清理边界按快照时间计算，至少保留一份跨越最近 24 小时边界的有效恢复点。这些目标尚待部署与恢复演练验证。

备份实现建议如下，具体工具配置和资源预算在实施时验证：

- 首版小规模 PostgreSQL 数据库优先评估每 30 分钟执行一次 `pg_dump` 一致性逻辑备份，使用自定义归档格式；完成后加密并上传独立私有 OSS 备份 Bucket，校验归档与远端文件后标记本次成功。备份解密材料另行保管，不能只保存在故障服务器上。
- 按最近一份已成功保存到 OSS 的有效备份所覆盖的快照时间计算备份年龄，不能用上传完成时间代替数据时间；可保守使用备份任务开始时间。调度间隔加上执行、上传和校验延迟需小于 1 小时，并为失败重试留出余量；失败后及时重试，失败或即将超出窗口时报告异常，具体通知渠道待定。
- 在目标主机验证备份对业务的影响与恢复耗时，并定期在隔离环境完整还原及检查关键业务数据。若逻辑备份的耗时或资源开销不再满足目标，再评估物理备份与 WAL 归档。
- 部署配置、数据库角色及必要的恢复凭据纳入恢复流程。OSS 业务文件在数据库备份之外保存；文件采用新对象 key 写入，上传成功并核验后才提交数据库引用，删除先撤销数据库中的有效引用，物理清理延后。清理规则必须保护所有保留中的数据库备份所引用的对象或版本，并覆盖 30 天历史窗口及恢复缓冲期，避免数据库还原后找不到其引用的文件；若依赖 OSS 历史版本恢复，数据库需保存对应 `versionId`。具体版本保护和生命周期规则在 OSS 接入时验证。

依据：[PostgreSQL 逻辑备份](https://www.postgresql.org/docs/current/backup-dump.html)、[`pg_dump` 说明](https://www.postgresql.org/docs/current/app-pgdump.html)。当前仅确定目标并提出方案，尚未执行真实备份或恢复演练。

## 已确认的代码与镜像发布

使用 GitHub 托管代码，由 GitHub Actions 执行检查并构建带版本标识的 Docker 镜像，推送 GHCR；管理者再通过 Portainer 手动选择版本部署，生产 VPS 运行已构建的服务。发布记录保留提交、版本与镜像 digest 的对应关系，便于追溯；数据库结构变更的兼容与恢复步骤随版本明确，镜像切换不代表自动回滚数据。仓库与镜像的可见性、实际远端地址，以及服务器拉取镜像的连通性和凭据在接入时确定。参考 [GitHub 镜像发布工作流](https://docs.github.com/en/actions/tutorials/publish-packages/publish-docker-images)。

## 已确认的统一工具链与质量检查

所有环境使用 mise 管理项目工具环境，后续项目命令统一经 mise 运行。Rust 配置 rustfmt 与 Clippy；前端配置 ESLint 与 Prettier。工具精确版本、锁文件、统一命令和检查失败语义在工程初始化时落实，本地与 CI 使用相同入口。此要求已纳入[实施规格](./spec.md)、[ADR 0011](../../docs/adr/0011-mise-unified-toolchain.md)和[首个工程基础任务](./issues/01-mise-toolchain-quality.md)；当前仍为文档阶段，尚未建立实际工程配置。

## 待讨论

- 对象存储的具体接入配置与 SDK 兼容验证。
- 登录实现的参数与依赖版本：会话有效期、密码哈希参数、登录与哈希并发限制；实施时验证。
- mise 统一工具链和契约生成工具的具体版本及兼容验证。
- 并发更新、接口约定、测试及分阶段实施任务，由实施规格统一给出建议。
- 备份工具配置与异常通知渠道，上线前确定。

## 讨论

- 2026-09-27：确认旧博客方案仅作参考，本项目没有历史业务数据需要迁移。
- 2026-09-27：选择 Rust DTO → OpenAPI → TypeScript 类型的方向，不强制使用 Protobuf。
- 2026-09-27：选择 PostgreSQL `BIGINT IDENTITY` 配合主键约束，不采用雪花 ID。
- 2026-09-27：确认前端采用 Vue 3 + TypeScript。
- 2026-09-27：确认使用管理者自行开发维护的 `@aifuxi/semi-ui-vue`，组件库可控及可按需演进是选型依据。
- 2026-09-27：确认后端采用 Axum + Tokio + SQLx + PostgreSQL，以模块化单体组织。
- 2026-09-27：确认部署到自己的服务器或 VPS；系统、配置和具体部署工具待确定。
- 2026-09-27：明确使用 Ubuntu，预计 2 核 CPU、4 GB 内存，从零搭建 Docker + Caddy 环境，并部署 Portainer 管理服务；要求调研 Caddy 是否由 Portainer 管理。
- 2026-09-27：确认采用部署调研建议：Caddy 容器化并由 Portainer 独立 Stack 管理，Portainer 本身由服务器 Compose 启动，保留面板外配置、持久化数据和 SSH 恢复入口。
- 2026-09-27：确认公开网站与管理端采用分子域名访问，主域名用于公开网站，`admin` 子域名用于管理端；示例域名不作为实际部署配置。
- 2026-09-27：确认管理者使用本地账号密码登录；服务端 Session 等配套实现仍为建议。
- 2026-09-27：确认采用 Argon2id 密码哈希与存储于 PostgreSQL 的服务端 Session，使用仅管理子域可用的安全 Cookie；通过服务器命令初始化及重置账号，退出撤销当前会话，改密或重置撤销全部会话。
- 2026-09-27：确认公开网站内容变更后立即生效，采用 Nuxt SSR 在请求时读取最新已发布内容；公开端在第二阶段加入。
- 2026-09-27：确认单仓库组织 Rust API、Vite 管理端与 Nuxt 公开端，各自独立构建和部署，前端通过 pnpm workspace 共享生成的接口类型。
- 2026-09-27：确认业务时区固定为 `Asia/Shanghai`，统一计算“今日”、习惯打卡、每日记录与每周回顾的日期边界。
- 2026-09-27：确认接入外部 S3 兼容对象存储，预期使用阿里云 OSS；区域、Bucket、SDK 与图片访问方式待后续确定。
- 2026-09-27：确认文章图片随文章权限变化，草稿图片私有，文章撤回后拒绝新的公开图片访问；配套采用私有对象存储与 Rust 媒体接口逐请求校验。
- 2026-09-27：确认服务器故障后从备份恢复时最多接受丢失最近 1 小时的数据（RPO ≤ 1 小时）；恢复服务时限与历史备份保留期待确认。
- 2026-09-27：确认服务器故障后 24 小时内恢复使用（RTO ≤ 24 小时），历史备份保留 30 天，最近 24 小时保留较密集的恢复点，更早每天保留一份。
- 2026-09-27：确认 GitHub Actions 构建并推送 GHCR，再由管理者通过 Portainer 手动选择版本部署；实际仓库和镜像接入信息后续提供。
- 2026-09-27：确认任务说明与每日记录采用 Markdown 加预览，删除任务或项目后保留已有完成历史；据此整理技术实施规格。
- 2026-09-27：要求全部环境使用 mise，所有后续项目命令经 mise 运行；配置 Rust 的 rustfmt、Clippy 及前端的 ESLint、Prettier。
