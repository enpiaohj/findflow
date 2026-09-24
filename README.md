# FindFlow

Windows 本地优先的极速文件搜索与轻量文件处理工具。全局快捷键（默认 Alt + Space）呼出紧凑悬浮窗，在不打断当前工作的前提下完成查找、筛选、预览、定位与常用文件操作——把"找到文件到完成动作"压缩到数秒以内。

> **当前状态**：基座重建阶段（`0.2.0` 开发中）。已完成项目脚手架、搜索窗外壳与 SQLite 设置持久化；索引引擎、全文搜索等能力按路线图逐步实现，当前结果列表使用演示数据。
>
> 历史版本 v0.1.0 及其源码已归档至 `D:\AIProjects\_Backup\2026-09-24-FindFlow-操作前备份-v1.0.bundle`，本仓库自 2026-09-24 起从零重建。

## 核心特性（路线图）

| 阶段 | 内容 |
| --- | --- |
| P1 MVP | 全局呼出悬浮窗、索引范围管理、初始与增量索引、文件名 / 路径 / 元数据搜索、打开 / 定位 / 复制路径、收藏与最近使用、设置与诊断 |
| P2 搜索工作台 | 全文搜索与命中片段、快速预览、批量重命名 / 复制 / 移动、大文件与重复文件分析、规则式智能整理 |
| P3 智能增强 | OCR 索引、离线自然语言查询、可选在线 LLM 解析、相似图片实验 |

产品原则：默认轻、按需重。默认只索引文件名与元数据，无管理员权限即可使用；全文、OCR、重复文件扫描均按需开启；删除默认进回收站；批量操作先预演再执行。

## 技术栈

| 层 | 选择 |
| --- | --- |
| 桌面容器 | Tauri 2 |
| UI | React 18 + TypeScript + Vite |
| 样式 | CSS Variables（Fluent 2 设计令牌，不引入重组件库） |
| 核心引擎 | Rust |
| 数据库 | SQLite（WAL 模式，rusqlite bundled） |
| 规划中 | Tantivy 全文索引、jieba-rs 中文分词、BLAKE3 哈希、windows-rs |

## 环境要求

- Node.js ≥ 20 与 npm
- Rust（MSVC 工具链）≥ 1.77
- WebView2 Evergreen Runtime（Windows 11 自带，Windows 10 由安装器检测）
- Windows 10 22H2 / Windows 11

## 开发

```bash
npm install
npm run tauri dev
```

## 构建与测试

```bash
npm run build          # 前端类型检查 + 构建
cd src-tauri
cargo test             # Rust 单元测试
cargo build            # Rust 编译
npm run tauri build    # 完整桌面打包（NSIS / MSI）
```

## 目录结构

```
src/          前端（React：搜索窗外壳、组件、样式）
src-tauri/    桌面端与核心引擎（Rust：数据库、设置持久化、后续索引与搜索引擎）
scripts/      辅助脚本（应用图标生成等）
releases/     正式发布快照（仅在正式发布时创建）
```

## 相关文档

- 产品设计文档与 UI 概念图：工作区 `D:\AIProjects\ai-coding-workspace\projects\FindFlow\01-产品设计\`
- 项目规则与发布约定：`CLAUDE.md`
- 安全策略：`SECURITY.md`
