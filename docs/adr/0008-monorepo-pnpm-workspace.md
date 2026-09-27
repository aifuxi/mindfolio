# 在单仓库中维护应用并通过 pnpm workspace 共享前端类型

本项目由唯一管理者维护，Rust API 的接口变化需要与两个前端协调，因此将 Rust API、Vite 管理端和 Nuxt 公开端放在同一仓库，各自独立构建和部署，便于同步修改接口与页面。前端使用 pnpm workspace 共享由 OpenAPI 生成的 TypeScript 接口类型，Rust 依赖和构建使用 Cargo；管理端采用 Vite 构建的 Vue SPA，由 Caddy 托管静态产物。公开端继续按已确认的分期在第二阶段实现，目录及具体版本在实施规格中确定。
