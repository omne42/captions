# 对 `gemini_caption` 的评估

原仓库位置：`/root/autodl-tmp/zjj/nieta/talesofai-repos/gemini_caption`

## 核心判断

值得重写，不值得继续补洞。

原因很简单：旧仓库的主要问题不是某个局部 bug，而是数据结构和数据流从一开始就没立住。你可以继续往里塞分支、塞重试、塞更多 Mongo 字段，但那只是把垃圾堆高一点。

## 结构观察

主要入口和相关文件：

- `src/gemini_batch_caption.py`
- `src/gemini_single_caption.py`
- `src/caption_promt_utils.py`
- `src/danbooru_pics.py`
- `src/danbooru_pics_normal.py`
- `src/character_analyzer.py`
- `src/scrapy/**`
- `src/input_db/**`

这套东西混了三类职责：

- 在线 caption 流水线
- Danbooru 元数据访问
- 一堆历史导入脚本和 Mongo 迁移脚本

结果就是仓库边界不清晰。真正跑 caption 的核心逻辑，被历史脚本、同步/异步双份实现、相对导入兼容逻辑包住了。

## 具体问题

### 1. 数据流过长，而且有重复 IO

旧流程大致是：

1. 查 Danbooru post JSON
2. 拿到图片 URL
3. 再把图片字节下载到本地内存
4. 再把字节传给 Gemini
5. 再把结果写入 Mongo

这里最蠢的地方，就是为了做远程视觉推理，先自己下载了一遍图片。完全没必要。直接把远程图片 URL 提交给模型接口就行，少一次下载，少一次内存占用，少一次失败点。

### 2. 缓存不是缓存，只是“跳过成功 ID”

在 `src/gemini_batch_caption.py` 里，所谓跳过逻辑本质上是：

- 先查某个 `_id`
- 如果 `success == True`，那就跳过

这不叫缓存。这只是把“处理状态”和“推理结果”糊在一份 Mongo 文档里。真正的缓存应该基于请求语义，而不是只基于 post id。

只要这些因素变化，结果就可能变：

- 模型
- prompt 版本
- 输出 schema
- 输出语言
- 图片 URL
- 元数据提示

旧实现没有把这些要素建模成指纹，所以缓存根本不可靠。

### 3. 同步/异步双份实现重复

`src/gemini_single_caption.py` 和 `src/gemini_batch_caption.py` 存在大量重复逻辑：

- 拉取 Danbooru URL
- 下载图片
- 调模型
- 写 Mongo
- 组织 prompt

这是典型的坏味道。两份实现不是能力扩展，只是重复维护。

### 4. 结果格式靠 `json_repair`

旧仓库依赖 `json_repair` 去兜模型输出。这说明接口层一开始就没把输出边界定义清楚。

如果你真的需要稳定结构化结果，就该：

- 用结构化输出
- 用 schema
- 让接口返回合法 JSON

而不是先让模型乱吐文本，再在下游修补。

### 5. Mongo 集合设计随 ID 分桶，业务语义差

旧仓库按 `dan_id // 100000` 动态分 collection。这个做法很像脚本时代的临时方案，不像一个真正稳定的在线流水线。

问题在于：

- 查询路径复杂
- 缓存和结果耦合
- 迁移困难
- 很难做统一统计

关系型表在这里明显更合适，因为这件事天然就是：

- `source_posts`
- `caption_jobs`
- `caption_cache`
- `caption_results`

## 品味评分

可接受，但粗糙。

不是完全乱写，至少能跑；但仓库缺少清晰的数据模型，导致所有补丁都只能长在边上，最终把特殊情况越堆越多。

## 改进方向

### 第一优先级：重写数据结构

先把这些概念拆开：

- 源数据
- 作业状态
- 指纹缓存
- 最新结果

一旦拆开，很多 `if/else` 会自然消失。

### 第二优先级：改为 URL 直传模型

别再手动下载图片字节。那是多余的数据拷贝。

### 第三优先级：改为结构化输出

不要在下游修 JSON。让模型按 schema 输出。

### 第四优先级：把 prompt cache 和本地 cache 分层

- OpenAI prompt cache 负责减少同前缀 prompt 的重复 token 开销。
- 本地 PG cache 负责真正的业务级去重和复用。

这两者不是替代关系，而是两层缓存。

## 最后的结论

旧仓库最大的问题不是“用了 Python”或者“用了 Mongo”。

真正的问题是：它没有把 caption 任务当成一个有清晰状态和缓存语义的数据管道，而是把它写成了一组能跑的脚本。

所以正确做法不是继续修，而是重写。
