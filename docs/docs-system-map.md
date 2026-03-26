# captions Docs System

## 入口分工

- `README.md`
  - 对外概览、快速开始和高层边界。
- `AGENTS.md`
  - 给执行者的短地图。
- `docs/`
  - 版本化事实来源。

## 目录职责

- `docs/architecture/`
  - `system-boundaries.md`：本仓负责什么、复用什么、不负责什么。
  - `source-layout.md`：源码树和文件职责。
- `docs/domains_and_boundaries.md`
  - 业务域与共享 foundation/runtime 边界说明。
- `docs/legacy_assessment.md`
  - 旧实现评估。
- `docs/architecture.mmd` / `docs/architecture.svg`
  - 架构图源文件与导出图。

## 新鲜度规则

- 业务域边界变化时，同时更新 `system-boundaries.md` 和 `domains_and_boundaries.md`。
- 入口文件或目录职责变化时，更新 `source-layout.md`。
- 架构图表达的关键流程变化时，同步更新 `architecture.mmd` 和导出图。
- 不把长期事实留在聊天记录里。
- `tests/docs_system.rs` 机械检查根入口和关键文档是否仍然存在。
