# FindFlow 项目规则

> 用户级全局规则（`~/.claude/rules/`）在本项目之上生效；本文件只定义项目专属规则。

## Repository Rule

- **产品名**：FindFlow
- **仓库**：`findflow`（GitHub：`enpiaohj/findflow`，Private，默认分支 `main`）
- **版本**：Semantic Versioning。当前 `0.2.0` 开发中。旧仓库历史中的 `v0.1.0` 已发布并随旧历史归档（见下），**不得再次发布 v0.1.0**；正式发布时递增 Build Number。
- **Commit**：Conventional Commits（`feat:` / `fix:` / `docs:` 等），描述用简体中文；发布提交格式 `release: FindFlow vX.Y.Z`；Tag 格式 `vX.Y.Z`。
- **发布产物**：安装包默认不入 Git History；正式发布时创建 `releases/vX.Y.Z/` 不可变快照（`source/` 与 `CHANGELOG.md` 入库，产物上传 GitHub Releases）。
- **设计文档**：产品设计文档与 UI 概念图维护于工作区 `D:\AIProjects\ai-coding-workspace\projects\FindFlow\01-产品设计\`，不入本仓库。
- **历史**：旧仓库（截至 `v0.1.0` 及其后 3 个提交）已于 2026-09-24 完整备份至 `D:\AIProjects\_Backup\2026-09-24-FindFlow-操作前备份-v1.0.bundle`，本仓库自零重建；未经用户明确授权不得覆盖远程历史。

## 技术栈

Tauri 2 + React 18 + TypeScript + Vite（前端）；Rust + rusqlite（SQLite WAL，数据基座）；规划中：Tantivy 全文索引、jieba-rs 中文分词、BLAKE3 哈希、windows-rs（USN / Shell API）。

## 常用命令

| 用途 | 命令 |
| --- | --- |
| 安装前端依赖 | `npm install` |
| 前端类型检查 + 构建 | `npm run build` |
| 桌面开发运行 | `npm run tauri dev` |
| 完整桌面打包 | `npm run tauri build` |
| Rust 单元测试 | `cd src-tauri && cargo test` |
| 生成应用图标源图 | `node scripts/generate-app-icon.mjs` |
| 生成 Tauri 图标集 | `npx tauri icon src-tauri/app-icon.png` |

## 产品闭环底线（来自设计文档 §10.2，任何阶段不得违反）

1. 删除默认进入 Windows 回收站；永久删除须二次确认且无全局快捷键。
2. 批量操作必须先预演再执行，逐项结果（成功 / 跳过 / 失败）可追溯。
3. 破坏性操作前用稳定 ID 与当前路径双重校验操作对象。
4. 基础搜索离线完整可用，不依赖 AI 与网络。
5. 禁止基于屏幕坐标的自动点击用于产品内部确认。
