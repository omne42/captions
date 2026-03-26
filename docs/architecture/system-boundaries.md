# 系统边界

## 目标

`captions` 是一个具体业务仓库：Danbooru 图片描述流水线。

它负责把“source post -> prompt/schema -> fingerprint -> cache -> generation -> result”这条业务链路跑通，而不是成为新的通用平台。

## 本仓负责什么

- Danbooru source adapter
- caption contract
- caption pipeline orchestration
- Postgres cache / result / job 数据流

## 复用什么

- `../omne_foundation/crates/config-kit/`
  - 配置文件发现、严格格式识别、`${ENV_VAR}` 插值与 typed schema loading。
- `../omne_foundation/crates/http-kit/`
  - 通用 HTTP client 和出站基础能力。
- `../omne-runtime/crates/omne-integrity-primitives/`
  - SHA-256 摘要原语。
- `../ditto-llm/crates/ditto-core/`
  - provider runtime、structured generation 和 Responses API 前门。

## 不负责什么

- 通用 caption pipeline foundation
- 通用作业系统
- 通用 HTTP / config / integrity 原语
- provider runtime / auth / adapter 层

## 不该下沉的部分

- Danbooru adapter
- Caption schema / prompt 语义
- PG cache/result/job 领域模型

这些边界离开 Danbooru 图片描述任务就失去意义，不应伪装成 shared foundation。
