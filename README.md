# FindFlow

Windows 本地优先的极速文件搜索与轻量文件处理工具。全局快捷键（默认 Alt + Q）呼出紧凑悬浮窗，在不打断当前工作的前提下完成查找、筛选、预览、定位与常用文件操作——把"找到文件到完成动作"压缩到数秒以内。

> **当前状态**：开发中，见 [Releases](../../releases) 获取最新安装包（Windows x64，NSIS / MSI）；版本变更见 `CHANGELOG.md`。
> 本项目以 [MIT](LICENSE) 许可证发布。

## 核心能力

- **全局呼出**：Alt + Q 呼出 / 隐藏（可在设置中改键；与其他软件冲突时自动降级到 Ctrl + Alt + Space 等备用键），失焦自动隐藏，单实例。
- **索引**：按目录配置范围（试扫确认、排除规则、系统目录保护），支持本地盘符与 UNC 网络共享路径（`\\服务器\共享`，网络范围不做实时监听、靠重新校准同步）；初始扫描 + 实时监听 + 定期校准；NTFS 文件标识稳定跟踪文件。
- **搜索**：文件名子串 / 拼音全拼 / 首字母 / 中文分词；筛选语法 `ext: type: path: size: modified: -排除 "短语"`；文件名通配符 `*.pdf`、`报告*.docx`、`2026-??-??`（`*` 任意多字符、`?` 单字符，按整名匹配）；综合相关度排序；全文（Office/PDF/文本/代码）与图片 OCR 按需开启。
- **动作**：打开、定位、复制路径（四种格式）、复制文件、重命名、回收站删除（永久删除需独立确认）、属性、终端 / VS Code。
- **批量**：重命名（预演 + 冲突检测 + 撤销）、移动（跨卷校验）、规则式智能整理。
- **分析**：大文件、重复文件（BLAKE3 三级扫描）、相似图片（dHash 实验）。
- **闭环**：批量先预演再执行、逐项结果可追溯、任务可暂停恢复取消、崩溃后任务标记 Interrupted 可续跑。

## 界面预览

> 以下截图使用示例数据渲染，仅用于展示界面。

| 搜索与列表（浅色） | 预览窗格（深色） |
| --- | --- |
| ![搜索与列表](docs/images/search-light.png) | ![预览窗格](docs/images/preview-dark.png) |

![菜单栏](docs/images/menubar-light.png)

## 快速上手

1. 安装后按 **Alt + Q** 呼出搜索窗；首次使用先在 设置 → 索引 中添加要索引的文件夹（支持本地盘符与 `\\服务器\共享`）。
2. 直接输入关键词搜索；可组合筛选语法：

| 写法 | 含义 |
| --- | --- |
| `报告 年度` | 多个词需同时命中 |
| `*.pdf`、`报告*.docx` | 文件名通配符（`*` 任意多字符、`?` 单字符） |
| `ext:docx`、`type:图片`、`-type:图片` | 按扩展名 / 类型筛选或排除 |
| `size:>100MB`、`modified:7d` | 按大小、最近修改时间 |
| `path:D:\Temp`、`-path:node_modules` | 限定或排除某个文件夹（绝对路径按目录边界匹配，路径含空格时写 `path:"D:\My Docs"`） |

3. 直接输入 `D:\Temp` 这样的完整路径，会列出该目录内容，**回车直接打开**该路径。
4. 常用按键：`↑↓` 选择，`Enter` 打开，`Ctrl+Enter` 在文件夹中定位，`Ctrl+C` 复制路径，`Ctrl+Shift+C` 复制文件，`Space` 预览，`Alt+P` 开关预览窗格，`Ctrl+L` 聚焦搜索框，`Esc` 清空 / 隐藏。
5. 菜单栏（文件 / 编辑 / 视图 / 搜索 / 工具 / 帮助）可用鼠标或 `Alt+F` 等打开；预览窗格默认关闭，可在「视图」菜单或筛选栏按钮开启。

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
cargo test             # Rust 单元测试
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

- 更新日志：`CHANGELOG.md`
- 安全策略：`SECURITY.md`
- 许可证：`LICENSE`（MIT）；第三方组件见 `THIRD-PARTY-NOTICES.md`
- 项目规则与发布约定：`CLAUDE.md`

## 历史

本仓库自 2026-09-24 从零重建；旧仓库的 v0.1.0 版本号永久退役，不再使用。
