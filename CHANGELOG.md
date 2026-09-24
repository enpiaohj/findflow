# FindFlow 更新日志

本文件记录自 2026-09-24 仓库从零重建后的变更。历史版本 v0.1.0 的变更记录随旧仓库历史归档于 `D:\AIProjects\_Backup\2026-09-24-FindFlow-操作前备份-v1.0.bundle`。

## 未发布

### Added

#### 仓库基线
- 仓库标准文件：README、CHANGELOG、CLAUDE.md 项目规则、AGENTS.md、SECURITY.md、.editorconfig、.gitattributes、.gitignore。
- Tauri 2 + React 18 + TypeScript + Vite 项目脚手架、自定义标题栏、应用图标生成脚本与全套图标。

#### 索引引擎（FR02 / FR03 / FR04）
- 索引范围管理：添加 / 启停 / 移除 / 试扫确认（含文件数与空间预估）、系统与程序目录保护、排除目录与扩展名。
- 初始扫描：低优先级目录遍历、批量事务写入、NTFS 文件标识（卷序列号 + 文件索引）、暂停 / 恢复 / 取消、失败计数不中断。
- 增量监听：ReadDirectoryChangesW（notify 封装）事件合并、重命名配对、缓冲区溢出与定期校准扫描（默认 30 分钟）。
- 重建指定范围、任务检查点与启动恢复（Running → Interrupted）。

#### 搜索（FR05 / FR06 / FR09 / FR16）
- SQLite（WAL）全量 Schema（volumes / scopes / files / content_state / favorites / tags / jobs / operation_log / index_outbox 等）与 user_version 迁移框架。
- Tantivy 倒排索引：jieba 中文分词 + 拼音全拼 / 首字母 + 2/3-gram 子串匹配；全文与 OCR 文本字段。
- 筛选语法：`ext:` `type:` `path:` `size:` `modified:` `-排除` `"短语"`；查询解析为统一 AST。
- 相关度排序：文本 45% / 完整命中 20% / 路径 10% / 最近打开 10% / 最近修改 8% / 收藏 5%（设计文档 §9）。
- 全文提取管道：TXT/代码（UTF-8/GBK 回退）、DOCX/XLSX/PPTX、PDF，50MB 上限、OneDrive 占位跳过、失败标记「仅文件名可搜」。
- 收藏、标签、最近使用（90 天保留）。
- 离线自然语言解析（今天修改的 Word / D盘大于2GB 等）+ 可选 DeepSeek 在线解析（Key 经 DPAPI 加密存储，仅发送查询文本）。

#### 文件动作与批量处理（FR07 / FR10 / FR11 / FR12）
- 打开、定位（Explorer /select）、复制路径（完整 / 文件名 / 父目录 / Markdown）、复制文件（CF_HDROP）、打开方式、属性、在终端 / VS Code 打开、单文件重命名。
- 删除默认进回收站；永久删除需短时确认令牌 + 二次确认；删除前重新校验目标。
- 批量重命名：查找替换 / 前后缀 / 日期 / 编号，预演（冲突 / 非法字符检测）→ 确认 → 逐项执行 → 一次性撤销（反向映射 + 状态校验）。
- 批量移动：跨卷复制 + 大小校验 + 删除源，冲突策略（跳过 / 覆盖 / 保留两者 / 仅较新）。
- 规则式智能整理（按扩展名 / 年月，默认仅建议）。

#### 空间分析（FR13 / FR14 / P3）
- 大文件分析（按大小阈值筛选）。
- 重复文件三级扫描：大小分组 → 头部快速指纹 → 完整 BLAKE3；分组展示与回收站清理。
- 相似图片实验：dHash 感知哈希 + 汉明距离（独立于字节级重复）。

#### 图片 OCR（FR17，可选）
- Windows.Media.Ocr 系统引擎，PNG/JPG/BMP，单图与每日处理量限制，文本独立字段参与搜索。

#### UI（v1.0 视觉基线）
- 快速搜索窗：紧凑 / 舒适 / 大图标三种密度（记忆上次选择）、筛选栏、排序菜单、Space 快速预览浮层（图片 / 文本 / Shell 缩略图 / 属性降级）、更多操作菜单、空结果建议、快捷键闭环（↑↓ / Space / Esc / Ctrl+L）、失焦自动隐藏、Alt+Space 全局呼出（可改键）、单实例。
- 完整管理窗：索引与搜索、空间分析、批量处理、任务与诊断、收藏与标签、外观与快捷键、隐私与安全、更新八个页面。
- 任务与诊断中心：运行中任务进度 / 暂停 / 取消、最近任务与失败重试、失败分类统计、诊断包导出、数据库完整性检查、日志目录（保留 14 天、路径脱敏）。

#### 更新（FR19）
- Tauri Updater + minisign 签名密钥（本地保管于 `D:\AIProjects\_Keys\`），检查 / 下载 / 安装闭环。

#### 性能基准工具
- `src-tauri/src/bin/bench_gen.rs`：生成 10 万 / 100 万条模拟元数据与 Tantivy 索引至 `.bench/`（已 gitignore），并输出查询 P50 / P95 / P99。

### Fixed

- 键盘闭环补全：Enter 打开（打开后窗口自动收回）、Ctrl+Enter 定位、Ctrl+C 复制路径、Ctrl+Shift+C 复制文件。
- 中文输入法（IME）合成期间不再拦截 ↑↓ / 空格 / Enter，选候选词与打字互不干扰。
- Space 仅在焦点不在搜索框时触发预览，修复多词查询无法输入空格的问题。
- 搜索窗失焦隐藏时清空搜索词与浮层（FR01：不恢复敏感搜索词），重新呼出自动聚焦输入框。
- 搜索请求加入 generation token，快速输入时过期响应不再覆盖新结果；搜索失败不再静默，给出提示。
- 扫描遍历阶段持续汇报「已发现 N 项」，不再长时间停留在 0%。
- NTFS 文件标识改用 FILE_READ_ATTRIBUTES 快速打开，避免全文件读取触发杀软扫描，扫描速度提升约一个数量级。
- 索引页范围状态与数量每 3 秒轮询刷新；排序菜单支持点击外部关闭。
- 移除 window-state 插件：修复关闭应用时管理窗可见状态被记住、下次启动自动弹出的问题。

- 修复管理窗空白：改为 tauri.conf.json 静态创建（启动隐藏），并以窗口 label 路由，不再依赖 URL 参数与运行时建窗。
- 修复所有 AppState 命令静默失败：命令签名统一为 State<Arc<AppState>>（此前注册类型与取用类型不一致，导致搜索、添加范围等全部无响应）。
- 修复 scopes 列表 baseline_at / files_count 列序映射错位（Invalid column type Null）。
- 修正试扫确认卡片的空间文案（此前把文件总大小误标为索引空间）。
- 修复 `PRAGMA user_version` 经 rusqlite `pragma_update` 写入不持久化的问题（改为 execute_batch 拼接），并补充跨连接回归测试。
- 修复窗口早于 AppState 注册时焦点事件导致的启动崩溃（`try_state` 守卫）。

### Verification
- Build：`npm run build`（tsc + vite）通过；`cargo build` / `cargo check --all-targets` 通过，无警告。
- Tests：`cargo test` 28 项全部通过（迁移 / 设置 / 查询解析 / 自然语言 / 分词 / 排除规则 / DPAPI 往返 / Schema 闭环）。
- Runtime：全功能版启动冒烟通过（日志无异常、窗口正常加载）；完整安装包（NSIS/MSI）构建未执行；100 万条查询基准见发布报告。
