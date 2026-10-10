# FindFlow

Windows 本地优先的极速文件搜索与轻量文件处理工具。全局快捷键（默认 Alt + Space）呼出紧凑悬浮窗，在不打断当前工作的前提下完成查找、筛选、预览、定位与常用文件操作——把"找到文件到完成动作"压缩到数秒以内。

> **当前状态**：`0.2.0` 开发版，设计文档 P1–P3 功能已实现主体。历史版本 v0.1.0 已归档（见下）。

## 核心能力

- **全局呼出**：Alt + Space 呼出 / 隐藏（可改键），失焦自动隐藏，单实例。
- **索引**：按目录配置范围（试扫确认、排除规则、系统目录保护），支持本地盘符与 UNC 网络共享路径（`\\服务器\共享`，网络范围不做实时监听、靠重新校准同步）；初始扫描 + 实时监听 + 定期校准；NTFS 文件标识稳定跟踪文件。
- **搜索**：文件名子串 / 拼音全拼 / 首字母 / 中文分词；筛选语法 `ext: type: path: size: modified: -排除 "短语"`；文件名通配符 `*.pdf`、`报告*.docx`、`2026-??-??`（`*` 任意多字符、`?` 单字符，按整名匹配）；综合相关度排序；全文（Office/PDF/文本/代码）与图片 OCR 按需开启。
- **动作**：打开、定位、复制路径（四种格式）、复制文件、重命名、回收站删除（永久删除需独立确认）、属性、终端 / VS Code。
- **批量**：重命名（预演 + 冲突检测 + 撤销）、移动（跨卷校验）、规则式智能整理。
- **分析**：大文件、重复文件（BLAKE3 三级扫描）、相似图片（dHash 实验）。
- **闭环**：批量先预演再执行、逐项结果可追溯、任务可暂停恢复取消、崩溃后任务标记 Interrupted 可续跑。

## 技术栈

| 层 | 选择 |
| --- | --- |
| 桌面容器 | Tauri 2（global-shortcut / single-instance / window-state / updater / dialog / opener 插件） |
| UI | React 18 + TypeScript + Vite，CSS Variables（Fluent 2 设计令牌） |
| 核心引擎 | Rust |
| 元数据 | SQLite（WAL，rusqlite bundled），user_version 迁移 |
| 全文索引 | Tantivy + jieba-rs + n-gram + 拼音 |
| 哈希 | BLAKE3（重复文件）、dHash（相似图片实验） |
| OCR | Windows.Media.Ocr（系统引擎，可选） |
| 自然语言 | 离线规则解析 + 可选 DeepSeek（Key 经 DPAPI 加密，仅发送查询文本） |

## 环境要求

- Node.js ≥ 20 与 npm
- Rust（MSVC 工具链）≥ 1.77
- WebView2 Evergreen Runtime
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
cargo test             # Rust 单元测试（28 项）
cargo build            # 桌面端编译
npm run tauri build    # 完整桌面打包（NSIS / MSI + 更新签名产物）
```

## 性能基准

```bash
cd src-tauri
cargo run --release --bin bench_gen -- --count 1000000          # 生成 100 万条元数据 + 倒排索引
cargo run --release --bin bench_gen -- --query "方案" --rounds 200  # 查询基准（P50/P95/P99）
```

数据位于 `.bench/`（已 gitignore，不入库）。删除 `.bench/` 即可完全重置。

## 目录结构

```
src/          前端（快速搜索窗 quick/、管理窗 manage/、共享组件）
src-tauri/    Rust 核心（扫描、监听、搜索索引、文件动作、批量、分析、OCR、NLQ、更新）
scripts/      辅助脚本（应用图标生成）
.bench/       性能基准数据（gitignore，本地生成）
releases/     正式发布快照（仅在正式发布时创建）
```

## 安全与隐私

- 本地优先：搜索完全本地完成，搜索词不落盘；基础功能不依赖网络与 AI。
- 删除默认进回收站；永久删除需短时确认令牌；系统目录受保护不可索引。
- API Key（可选）经 Windows DPAPI 加密存储于本机，不进代码与仓库。

## 相关文档

- 产品设计文档与 UI 概念图：工作区 `D:\AIProjects\ai-coding-workspace\projects\FindFlow\01-产品设计\`
- 项目规则与发布约定：`CLAUDE.md`
- 更新日志：`CHANGELOG.md`
- 安全策略：`SECURITY.md`

## 历史

旧仓库（截至 v0.1.0 及其后 3 个提交）已于 2026-09-24 完整备份至 `D:\AIProjects\_Backup\2026-09-24-FindFlow-操作前备份-v1.0.bundle`，本仓库自零重建；v0.1.0 版本号永久退役。
