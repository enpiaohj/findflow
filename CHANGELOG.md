# FindFlow 更新日志

本文件记录自 2026-09-24 仓库从零重建后的变更。历史版本 v0.1.0 的变更记录随旧仓库历史归档于 `D:\AIProjects\_Backup\2026-09-24-FindFlow-操作前备份-v1.0.bundle`。

## 未发布

### Added

- 仓库标准文件：README、CHANGELOG、CLAUDE.md 项目规则、AGENTS.md、SECURITY.md、.editorconfig、.gitattributes、.gitignore。
- Tauri 2 + React 18 + TypeScript + Vite 项目脚手架与自定义标题栏悬浮搜索窗外壳。
- 搜索窗三种显示密度（紧凑 / 舒适 / 大图标），密度选择通过 SQLite 设置持久化。
- SQLite（WAL）数据基座：连接配置、`user_version` 迁移框架、settings 表与 `get_setting` / `set_setting` / `delete_setting` 命令。
- Rust 单元测试覆盖设置读写与迁移流程。
- 应用图标生成脚本 `scripts/generate-app-icon.mjs` 与 `src-tauri/icons` 图标集。
