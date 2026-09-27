# 从 Rust DTO 派生接口契约与 TypeScript 类型

本项目从零建设，旧博客方案仅作技术参考，不继承其中 Protobuf 必选的要求。采用 Rust 请求与响应 DTO → OpenAPI → TypeScript 类型的契约生成方向，接口使用 REST/JSON，以减少重复维护消息结构和 Protobuf 生成链的维护成本。具体生成库、版本及运行时校验方案在工程设计时确定。
