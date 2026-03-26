# 源码布局

## 入口文件

- `src/main.rs`
  - CLI 入口。
- `src/lib.rs`
  - 模块装配与导出入口。

## 领域文件

- `src/config.rs`
  - 业务配置 schema 与 `config-kit` 接线。
- `src/danbooru.rs`
  - Danbooru source adapter。
- `src/prompt.rs`
  - caption prompt、schema 和请求指纹语义。
- `src/pipeline.rs`
  - 业务流水线编排。
- `src/store.rs`
  - Postgres 持久化与 cache/result/job 数据访问。
- `src/openai.rs`
  - 对 `ditto-core` 的业务侧装配。
- `src/models.rs`
  - 本仓业务模型。
- `src/error.rs`
  - 错误类型。

## 文档与迁移

- `docs/`
  - 记录系统入口。
- `migrations/`
  - 数据库迁移。

## 布局约束

- 保持按业务领域拆分，不新增语义模糊的 `utils`/`misc` 桶。
- 通用能力优先复用 sibling foundation/runtime，而不是回到业务仓手写。
