; FindFlow 安装钩子（由 tauri.conf.json -> bundle.windows.nsis.installerHooks 引用）
;
; 「设置 -> 应用」卸载列表里的名称带上版本号：FindFlow v0.8.0。
; 不改 productName：它同时决定卸载注册表键、安装目录和快捷方式名，加版本号会让每个版本
; 各装一份、无法原地升级。这里只改显示名；每次安装（含覆盖升级）都会刷新为当前版本号。
; DisplayVersion（版本列）由安装器自己写入，保持不变。

!macro NSIS_HOOK_POSTINSTALL
  WriteRegStr SHCTX "${UNINSTKEY}" "DisplayName" "${PRODUCTNAME} v${VERSION}"
!macroend
