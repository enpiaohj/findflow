# FindFlow v0.4.1

发布日期：2026-09-27

> 两处体验修复：设置页版本号显示、打开文件返回后搜索结果丢失。

## Fixed
- 「设置 → 更新」页当前版本号此前写死为 v0.2.0，与实际版本不一致；现与标题栏一致，从应用元数据实时读取。
- 搜索结果中打开文件、关闭后回到搜索窗，搜索词与结果被清空（原为失焦即清词的隐私行为）。现默认保留上次搜索；如需原隐私行为，可在「外观与快捷键 → 隐藏时清空搜索词」开启。

## Known Issues
- 仓库为 Private，应用内「检查更新」无法访问更新端点（404），升级需手动下载安装包覆盖安装。
- 本版本未变更索引格式，v0.4.0 升级后无需重建索引。

## Verification
- Build：`npm run tauri build`（release，含更新签名）通过；前端 `tsc` + `vite build` 通过
- Tests：`cargo test` 41 项全部通过
- 运行验证：打包产物实机启动检查——「更新」页显示 v0.4.1；「外观与快捷键」出现「隐藏时清空搜索词」开关且默认关闭
- 未执行：安装包实机安装 / 卸载测试、打开文件后返回场景的端到端自动化验证
- Platform：Windows / Architecture：x64

## Artifacts
- FindFlow-v0.4.1-win-x64-setup.exe（NSIS 安装包，自动更新使用）
- FindFlow-v0.4.1-win-x64-setup.exe.sig（更新签名）
- FindFlow-v0.4.1-win-x64.msi
- latest.json / SHA256SUMS.txt

## Git
- Tag：v0.4.1（指向提交 `release: FindFlow v0.4.1`）
