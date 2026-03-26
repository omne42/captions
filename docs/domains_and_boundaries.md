# captions Domains And Boundaries

`captions` 不是一个新的基础设施仓库，它是一个具体业务流水线。

因此先把边界说清楚，否则很容易把本来该复用的基础能力又在业务仓库里手写一遍。

## 本仓库负责的领域

### 1. Danbooru source adapter

负责：

- Danbooru post JSON 拉取
- 可用图片 URL 提取与归一化
- Danbooru tag/rating/source 元数据整理

不负责：

- 通用 HTTP client 能力
- 通用 JSON 配置读取
- 通用图片下载/缓存框架

这是 `captions` 自己的业务输入适配层，应该留在仓库内。

### 2. Caption contract

负责：

- caption prompt 文本
- 结构化输出 schema
- `CaptionPayload` 语义与 validate
- 请求语义指纹的组成字段

不负责：

- provider runtime 装配
- MCP / notify / policy 等其他基础域
- 通用 prompt 目录运行时

这里虽然包含 prompt 和 schema，但语义仍然是“Danbooru 图片描述任务长什么样”，不是通用 foundation。

### 3. Caption pipeline orchestration

负责：

- `post -> source -> fingerprint -> cache -> generation -> result` 的业务数据流
- `caption_jobs` / `caption_cache` / `caption_results` 的写入顺序
- range 并发执行和失败统计

不负责：

- 通用作业系统
- 通用 PG ORM / repository framework
- 通用 LLM agent orchestration

这是业务流水线本身，不能为了抽象而下沉。

## 应该复用的基础域

### 1. Config input layer

这部分已经改为复用 `omne_foundation/config-kit`。

原因：

- 配置文件发现、格式识别、严格解析、`${ENV_VAR}` 插值，本来就是通用 config 边界
- `captions` 不应该再手写 `read_to_string + toml::from_str`
- `config-kit` 已经提供了业务 schema 之上的 typed loader

当前边界：

- `captions` 自己定义配置 schema
- `config-kit` 负责文件发现、JSON/TOML/YAML 解析、layer merge 和 env interpolation

### 2. HTTP outbound foundation

这部分继续复用 `omne_foundation/http-kit`。

原因：

- bounded body read
- shared reqwest client construction
- upstream error 收口

`captions` 只保留 Danbooru API 语义，不复制 HTTP 基础能力。

### 3. Integrity / fingerprint primitives

这部分改为复用 `omne-runtime/omne-integrity-primitives`。

原因：

- SHA-256 计算不是 caption 业务语义
- digest 原语已经在 runtime 层存在
- 业务仓库不该继续散着写 `sha2 + hex`

`captions` 仍然决定“指纹里包含哪些字段”，但不再自己实现 SHA-256 原语。

### 4. Provider runtime / structured generation

这部分继续复用 `ditto-core`。

原因：

- `build_language_model(...)`
- provider auth / adapter / Responses API frontdoor
- `ResponseFormat::JsonSchema`

`captions` 只拼业务 request，不自己下沉 provider runtime。

## 当前不应新建 foundation 的部分

下面这些目前还不该抽成新基建：

- 通用 Danbooru crate
- 通用 caption cache store crate
- 通用 caption pipeline crate
- 通用 prompt fingerprint crate

原因很简单：现在还看不到稳定、跨仓库复用的边界，强抽只会制造新垃圾层。

## 一句话总结

`captions` 应该保留“Danbooru 图片描述流水线”的业务语义，把配置、HTTP、完整性原语和 provider runtime 明确交给 `omne_foundation` / `omne-runtime` / `ditto-core`。
