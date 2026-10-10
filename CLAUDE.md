# FindFlow 项目规则

> 用户级全局规则（`~/.claude/rules/`）在本项目之上生效；本文件只定义项目专属规则。`AGENTS.md` 指向本文件，不再单独维护。

## Repository Rule

- **产品名**：FindFlow
- **仓库**：`findflow`（GitHub：`enpiaohj/findflow`，**Public**，MIT 许可证，默认分支 `main`）。2026-10-10 由用户决定转为公开；不得擅自修改可见性。
- **版本**：Semantic Versioning。当前 `v0.8.1` 已发布（2026-10-10），后续开发基于此。旧仓库历史中的 `v0.1.0` 已发布并随旧历史归档（见下），**不得再次发布 v0.1.0**；已发布版本见 `releases/` 目录。
- **Commit**：Conventional Commits（`feat:` / `fix:` / `perf:` / `docs:` / `build:` / `test:` 等），描述用简体中文；发布提交格式 `release: FindFlow vX.Y.Z`；Tag 格式 `vX.Y.Z`。
- **提交身份**：本仓库只用 GitHub noreply 地址 `250103182+enpiaohj@users.noreply.github.com` 提交（本地 git 已配置）。用户明确要求「不体现邮箱地址」，任何提交、tag、文档中都不得出现真实邮箱。
- **发布产物**：安装包不入 Git History；正式发布时创建 `releases/vX.Y.Z/` 不可变快照（`source/`、`CHANGELOG.md`、`SHA256SUMS.txt`、`latest.json`、`.sig` 入库，安装包上传 GitHub Releases）。
- **GitHub Releases 自 v0.7.1 起**：`v0.2.0`–`v0.7.0` 的 Release 不再存在（用户：「之前的基本都半成品」）。这些旧 tag 仍保留在本地，**不要推送**，也不要重建对应 Release。发布新版本时只推送新 tag（`git push origin vX.Y.Z`），不要用 `git push --tags`。
- **设计文档**：产品设计文档与 UI 概念图维护于仓库之外的工作区，不入本仓库；`docs/superpowers/`（内部设计 / 实施计划）已 gitignore，不跟踪。
- **历史**：
  - 旧仓库（截至 `v0.1.0` 及其后 3 个提交）已于 2026-09-24 完整备份，本仓库自零重建。
  - 2026-10-10 公开前：为去除提交里的个人邮箱并移除内部设计文档，重写了全部历史，并删除、同名重建了 GitHub 仓库（强制推送后旧提交仍可按提交号访问，只有重建才能清除）。重写前的历史与旧 Release 附件都有本地备份（见下）。
  - 未经用户明确授权不得覆盖远程历史、强制推送或删除仓库 / Release / tag。
- **本地备份位置**（`D:\AIProjects\_Backup\`，不入库）：`2026-09-24-FindFlow-操作前备份-v1.0.bundle`、`2026-10-10-FindFlow-历史重写前备份-v1.0.bundle`、`2026-10-10-FindFlow-重建仓库前备份-v1.0.bundle`、`2026-10-10-FindFlow-GitHub发布附件备份-v1.0\`。

## 技术栈

Tauri 2 + React 18 + TypeScript + Vite（前端）；Rust（核心）；SQLite（rusqlite，WAL，`user_version` 顺序迁移，当前 v6）；Tantivy 倒排索引 + jieba-rs 中文分词 + n-gram + 拼音；BLAKE3（重复文件）；windows-rs（Shell API、DPAPI、OCR）。

## 常用命令

| 用途 | 命令 |
| --- | --- |
| 安装前端依赖 | `npm install` |
| 前端类型检查 + 构建 | `npm run build` |
| 前端单元测试 | `npm run test:unit` |
| 桌面开发运行 | `npm run tauri dev` |
| 完整桌面打包 | `npm run tauri build` |
| Rust 单元测试 | `cd src-tauri && cargo test` |
| Rust 静态检查（要求 0 警告） | `cd src-tauri && cargo clippy --all-targets` |
| 生成全套应用图标（源文件 `src-tauri/icons/source/*.svg`，需本机 Chrome 或 Edge） | `node scripts/generate-app-icon.mjs` |
| 性能基准（生成 100 万条，去重后约 85 万） | `cd src-tauri && cargo run --release --bin bench_gen -- --count 1000000` |
| 性能基准（查询 P50/P95/P99，与界面同一路径） | `cd src-tauri && cargo run --release --bin bench_gen -- --query "关键词" --rounds 200` |
| 改写 MSI 卸载列表显示名（发布时用） | `powershell -NoProfile -File scripts\msi-set-display-name.ps1 -Path <msi> -Name "FindFlow vX.Y.Z"` |

## 架构要点（改动前先了解）

- **搜索**（`src-tauri/src/search.rs`）：有关键词时 Tantivy 给候选、SQLite 过滤补全、综合排序；没有关键词的纯筛选查询走「沿排序索引边走边筛」快路径（给筛选列加一元 `+`，凑够条数即停），带 25ms 时间预算（SQLite 进度回调，rusqlite `hooks` 特性），超时退回常规计划并在 60 秒内不再重试同一查询。两种计划必须返回完全相同的条目与顺序（排序末尾追加同方向 `file_id`，有测试逐条比对）。
- **不要为筛选查询新增 `(kind, …)` / `(ext, …)` 复合索引**：v0.8.1 实测 15 种组合，凡有效的都会让优化器在其它排序下选错索引（「文档按名称排序」390ms → 1.7s）。
- **命中数**：纯筛选的计数限时 20ms，超时返回 `matchedPending`，前端显示「N+」并调用 `cmd_search_count` 取精确值。
- **索引范围开关**：`scopes.enabled=0` 的范围不参与搜索与计数（仅在存在已关闭范围时才追加条件）；开关 / 移除时要清总数缓存与健康统计缓存，重新启用时补一次校准。
- **索引一致性**（`outbox.rs` / `searchindex.rs`）：SQLite 事务先落元数据与 outbox 事件，后台再提交 Tantivy；事件在提交成功前不删除。Tantivy 写入线程遇 I/O 错误会使 `IndexWriter` 永久失效，必须丢弃重建并把「已写入未提交」的事件重置为待处理重放。
- **网络共享**（`scanner.rs`）：UNC 范围不取文件标识、不预统计，4 线程并行读目录、条目处理与写库仍串行；不做实时监听，靠校准。
- **破坏性操作**（`actions.rs`）：一次性确认令牌（系统 CSPRNG，绑定请求指纹，60 秒有效）+ 稳定 `file_id` 与当前路径双重校验；`recycleItem`（前端）是右键菜单与菜单栏共用的唯一入口。
- **前端纯逻辑**放在 `src/*.ts`（`layout`、`highlight`、`searchScope`、`searchHistory`、`startView`）并配单元测试；组件里不要内联可测试的逻辑。
- **样式**：颜色、间距、字号、圆角、动效一律用 `src/styles.css` 顶部的设计令牌，不要硬编码；浮层用 `--surface-pop` / `--border-pop` / `--shadow-pop`。

## 测试与环境注意

- **杀毒软件噪声**：本机 Windows Defender 实时扫描会让 Tantivy 写新建索引文件时偶发 `PermissionDenied`（实测默认临时目录约 15%，Defender 排除目录 0%）。会真实创建索引的测试已用 `crate::testutil::retry_on_index_io` 包裹（只重试这类瞬时 I/O 错误）；新增此类测试时照做。
- **脚本写文件的坑**：用 Python / sed / heredoc 向源码写入含反斜杠的内容容易丢转义或被当成转义（如 `\2026`）；含反斜杠的修改优先用编辑工具，或用原始字符串并事后核对。
- **界面验证**：可用 `dist/` + 给 `window.__TAURI_INTERNALS__` 打桩 + Edge 无头模式渲染截图；注意无头环境带「减少动效」且视口比窗口尺寸矮约 92px。这只是渲染验证，不能替代真机点击。
- **不要在本机实际安装测试包**：会覆盖用户现有 FindFlow 的卸载注册表项；安装器只做静态验证，实机安装由用户进行。

## 发布流程要点（在全局 release 规则之上）

1. 工作区干净；跑完整验证：`npm run build`、`npm run test:unit`、`cargo test`、`cargo clippy --all-targets`。
2. 版本号四处同步：`package.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`（`Cargo.lock` 由构建更新）、本文件的「版本」行；根目录 `CHANGELOG.md` 新增条目，`README.md` 的版本行与基准数字按需更新。
3. `npm run tauri build`。本环境自动签名会报找不到私钥（构建产物已生成，不影响），改为手动签名：`npx tauri signer sign -f <私钥> -p <密码> <setup.exe>`。
4. 产物复制到 `releases/vX.Y.Z/` 并规范命名：`FindFlow-vX.Y.Z-win-x64-setup.exe`（来自 `bundle/nsis/FindFlow_X.Y.Z_x64-setup.exe`）、`FindFlow-vX.Y.Z-win-x64.msi`（来自 `bundle/msi/FindFlow_X.Y.Z_x64_zh-CN.msi`）。
5. **对 MSI 副本执行显示名脚本**（见常用命令），必须在计算 SHA256 之前，且不要改 `target\` 下的原始产物。
6. 生成 `SHA256SUMS.txt`、`latest.json`（签名取自 `.sig`，下载地址指向本版本 Release）、`source/` 快照（`git ls-files -co --exclude-standard`，排除 `releases/` 与 `.claude/`）；检查快照内无密钥。
7. 提交 `release: FindFlow vX.Y.Z` → `git tag -a vX.Y.Z` → `git push origin main` → `git push origin vX.Y.Z`（只推这一个 tag）→ `gh release create` 上传 5 个文件 → 下载回来核对 SHA256。

## 安装包语言与显示名

- NSIS 为简体中文（`SimpChinese`），MSI 为 `zh-CN` 且使用自定义资源 `src-tauri/windows/wix-zh-CN.wxl`（Tauri 内置的中文资源只翻译了语言代码）。
- 「设置 → 应用」卸载列表里的名称带版本号（`FindFlow vX.Y.Z`）。`productName` 保持 `FindFlow` 不变：它同时决定卸载注册表键、安装目录与快捷方式名，带版本号会让每个版本各装一份、无法原地升级。
- NSIS 由 `src-tauri/windows/installer-hooks.nsh` 在安装后写入显示名（每次覆盖升级自动刷新）；MSI 无对应配置，靠发布流程第 5 步的脚本。
- 应用内目前**没有「检查更新」入口**（v0.6.0 因仓库私有而移除）；后端更新命令与 `latest.json` 仍在维护，仓库公开后下载地址已可匿名访问，恢复入口属于待办而非现状。

## 密钥与敏感信息

- DeepSeek API Key：DPAPI 加密存于 `%APPDATA%\com.findflow.app\llm.key`，不进仓库；用户可在「隐私与安全」中自行粘贴或更换。
- 更新签名私钥：保存在仓库之外的本机密钥目录（密码见该目录说明文件），禁止入库，也不要在提交说明、文档或日志里写出密码。
- 仓库是公开的：新增文档与示例时不要写入本机用户名、内网地址、个人邮箱等信息。

## 产品闭环底线（来自设计文档 §10.2，任何阶段不得违反）

1. 删除默认进入 Windows 回收站；永久删除须二次确认且无全局快捷键。
2. 批量操作必须先预演再执行，逐项结果（成功 / 跳过 / 失败）可追溯。
3. 破坏性操作前用稳定 ID 与当前路径双重校验操作对象。
4. 基础搜索离线完整可用，不依赖 AI 与网络。
5. 禁止基于屏幕坐标的自动点击用于产品内部确认。
