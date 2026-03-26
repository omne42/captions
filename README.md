# captions

一个重写版的 Danbooru 图片描述流水线。

这次不再修旧的 `gemini_caption`。那个实现的问题不在于“能不能跑”，而在于数据流太乱：先下载图片、再拼一大段 prompt、再把结果散着写进 Mongo，所谓缓存基本只是“查一下这个 ID 有没有成功过”。这很粗糙。

现在这个仓库改成了：

- Rust 2024
- 复用 `ditto-llm` 的 L0 能力（`ditto-core`）
- OpenAI `Responses API` 通过 `ditto-core::runtime::build_language_model(...)` 前门装配后调用
- 复用 `omne_foundation/config-kit` 处理业务配置文件发现、严格格式解析和 `${ENV_VAR}` 插值
- 复用 `omne_foundation/http-kit` 处理 Danbooru 出站 HTTP
- 复用 `omne-runtime/omne-integrity-primitives` 处理请求指纹的 SHA-256 原语
- PostgreSQL 持久化
- 两层缓存
  - 本地 `Postgres` 指纹缓存
  - OpenAI `prompt_cache_key`
- 优化后的数据流
  - 直接把 Danbooru 图片 URL 传给模型
  - 不再先下载图片字节
  - 统一作业状态、缓存和结果表

## 快速开始

1. 准备环境变量

```bash
export OPENAI_API_KEY="sk-..."
export CAPTIONS_DATABASE_URL="postgres://user:password@127.0.0.1:5432/captions"
```

也可以写配置文件，默认发现顺序：

- `./captions.toml`
- `./captions.yaml`
- `./captions.json`
- `./.captions/config.toml`
- `./.captions/config.yaml`
- `./.captions/config.json`

例如：

```toml
database_url = "${CAPTIONS_DATABASE_URL}"
model = "gpt-4.1"
language = "zh"
request_timeout_secs = 60
prompt_cache_namespace = "captions"
```

2. 初始化数据库

```bash
cargo run -- migrate
```

3. 处理单个 post

```bash
cargo run -- post --post-id 634678
```

4. 批量处理区间

```bash
cargo run -- range --start-id 634670 --end-id 634690 --concurrency 4
```

## 可配置参数

可以通过命令行或环境变量控制：

- `OPENAI_API_KEY`
- `CAPTIONS_DATABASE_URL`
- `DATABASE_URL`
- `CAPTIONS_OPENAI_BASE_URL`
- `CAPTIONS_MODEL`
- `CAPTIONS_LANGUAGE`
- `CAPTIONS_MAX_OUTPUT_TOKENS`
- `CAPTIONS_REQUEST_TIMEOUT_SECS`
- `CAPTIONS_PROMPT_CACHE_NAMESPACE`
- `--config <path>`：显式指定 JSON / TOML / YAML 配置文件
- `--model`，默认 `gpt-4.1`
- `--language`，默认 `zh`
- `--openai-base-url`，默认 `https://api.openai.com/v1`
- `--prompt-cache-namespace`
- `--request-timeout-secs`
- `--max-output-tokens`

## 数据流

![Architecture](docs/architecture.svg)

Mermaid 源文件在 [docs/architecture.mmd](/root/autodl-tmp/zjj/p/captions/docs/architecture.mmd)。

核心流程：

1. 从 Danbooru 拉取 post JSON，并抽出图片 URL 与标签。
2. 用 `prompt + image_url + schema + model + language` 计算请求指纹。
3. 先查 `caption_cache`，命中就直接回填 `caption_results`，不再调用模型。
4. 未命中时，通过 `ditto-core runtime frontdoor -> L0 OpenAI adapter` 调用 `Responses API`，直接提交图片 URL。
5. 将结构化输出、原始请求/响应、usage、作业状态全部写入 Postgres。

## 表结构

- `source_posts`
  保存 Danbooru 原始元数据和归一化后的图片 URL / tag 列表。
- `caption_cache`
  以请求指纹为主键，保存请求体、响应体、结构化 caption 和 token usage。
- `caption_results`
  保存每个 post 当前最新的产出，适合业务读取。
- `caption_jobs`
  保存作业执行状态，便于排查失败和统计缓存命中。

## 旧仓库评估

完整评估见 [docs/legacy_assessment.md](/root/autodl-tmp/zjj/p/captions/docs/legacy_assessment.md)。

一句话结论：

- 品味评分：可接受，但实现粗糙。
- 致命问题：把“是否处理过”“缓存”“结果存储”混成一个 Mongo 文档状态。
- 最大改进：把数据结构拆干净，让缓存成为普通路径，而不是异常分支。

## 设计取舍

- 直接使用图片 URL，而不是先下载图片字节。
  这是最直接的优化，少一次网络 IO，少一份内存拷贝。
- 复用 `ditto-core` 的 runtime frontdoor 和 L0，而不是在业务仓库手写 OpenAI 请求体或直接 `OpenAI::new(...)`。
  这才是正确边界。`captions` 负责业务数据流，runtime/L0 负责 provider 装配、调用与协议适配。
- 保留 OpenAI prompt caching，但不把它当唯一缓存。
  真正可靠的是本地 Postgres 指纹缓存，OpenAI cache key 只是加速器。
- 结果结构化输出。
  旧仓库要 `json_repair`，这就是接口设计不干净。现在直接用 JSON schema 约束输出。

## 领域与边界

补充说明见 [docs/domains_and_boundaries.md](/root/autodl-tmp/zjj/p/captions/docs/domains_and_boundaries.md)。

当前边界是：

- `captions` 自己保留 Danbooru source adapter、caption contract、PG cache/result/job 数据流
- 通用配置输入层复用 `config-kit`
- 通用 HTTP 出站层复用 `http-kit`
- SHA-256 指纹原语复用 `omne-integrity-primitives`
- provider runtime / structured generation 继续复用 `ditto-core`

## 文档入口

这个仓库采用短入口 + 分层事实文档：

- [AGENTS.md](/root/autodl-tmp/zjj/p/captions/AGENTS.md)
- [docs/README.md](/root/autodl-tmp/zjj/p/captions/docs/README.md)
- [docs/docs-system-map.md](/root/autodl-tmp/zjj/p/captions/docs/docs-system-map.md)
- [docs/architecture/system-boundaries.md](/root/autodl-tmp/zjj/p/captions/docs/architecture/system-boundaries.md)
- [docs/domains_and_boundaries.md](/root/autodl-tmp/zjj/p/captions/docs/domains_and_boundaries.md)

## 验证

开发时建议固定跑：

```bash
cargo fmt
cargo test
cargo clippy --all-targets -- -D warnings
```
