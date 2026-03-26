# captions AGENTS Map

这个文件只做导航。稳定事实写在 `README.md` 和 `docs/`。

## 先看哪里

- 外部概览：`README.md`
- 文档入口：`docs/README.md`
- 文档系统地图：`docs/docs-system-map.md`
- 系统边界：`docs/architecture/system-boundaries.md`
- 源码布局：`docs/architecture/source-layout.md`
- 业务域与共享边界：`docs/domains_and_boundaries.md`

## 修改规则

- `AGENTS.md` 保持短小，不把业务细节堆进来。
- 业务域与共享 foundation / runtime 边界变化时，更新 `docs/architecture/system-boundaries.md` 和 `docs/domains_and_boundaries.md`。
- 模块职责变化时，更新 `docs/architecture/source-layout.md`。

## 验证

- `cargo fmt`
- `cargo test`
- `cargo clippy --all-targets -- -D warnings`
