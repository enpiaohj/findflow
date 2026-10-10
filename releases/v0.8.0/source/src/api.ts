import { invoke } from "@tauri-apps/api/core";
import { isEnabled as autostartIsEnabledRaw, enable as autostartEnableRaw, disable as autostartDisableRaw } from "@tauri-apps/plugin-autostart";

export interface FileRow {
  fileId: number;
  name: string;
  path: string;
  ext: string;
  kind: string;
  size: number;
  modifiedMs: number;
  createdMs: number;
  isDir: boolean;
  fulltextState: string;
  snippet: string | null;
  score: number;
  favorite: boolean;
  tags: string[];
}

export interface SearchResponse {
  items: FileRow[];
  /** 命中总数；只返回前 N 条时用它说明实际有多少条命中 */
  matched: number;
  /** 索引总数（页脚「索引 N 条」） */
  total: number;
  elapsedMs: number;
  explain: string[];
}

export interface ScopeInfo {
  scopeId: number;
  path: string;
  enabled: boolean;
  state: string;
  filesCount: number;
  baselineAt: string | null;
  optionsJson: string;
}

export interface JobInfo {
  jobId: number;
  kind: string;
  state: string;
  progress: number;
  message: string | null;
  resultJson: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface IndexHealth {
  totalFiles: number;
  fulltextDone: number;
  fulltextFailed: number;
  ocrDone: number;
  ocrFailed: number;
  outboxPending: number;
  dbSizeBytes: number;
  scopes: ScopeInfo[];
  lastCheckpointMs: number;
}

export interface PreviewPayload {
  kind: "image" | "text" | "thumbnail" | "none";
  path: string;
  name: string;
  size: number;
  modifiedMs: number;
  data: string | null;
  reason: string | null;
}

export interface PlanItem {
  fileId: number;
  oldPath: string;
  newPath: string;
  status: string;
  reason: string | null;
}

export const getSetting = <T,>(key: string): Promise<T | null> =>
  invoke("get_setting", { key });
export const setSetting = (key: string, value: unknown): Promise<void> =>
  invoke("set_setting", { key, value });

export const search = (
  query: string,
  sort?: string,
  limit?: number,
  scopeId?: number
): Promise<SearchResponse> => invoke("cmd_search", { query, sort, limit, scopeId });

export const nlParse = (text: string): Promise<{ ast: unknown; confidence: number; explain: string[] }> =>
  invoke("cmd_nl_parse", { text });
export const nlLlmParse = (text: string): Promise<{ explain?: string[] }> =>
  invoke("nl_llm_parse", { text });
export const llmHasKey = (): Promise<boolean> => invoke("llm_has_key");
export const llmSetKey = (key: string): Promise<void> => invoke("llm_set_key", { key });
export const llmClearKey = (): Promise<void> => invoke("llm_clear_key");

export const scopesList = (): Promise<ScopeInfo[]> => invoke("scopes_list");
export const scopeAdd = (
  path: string,
  options?: unknown,
  filesEst?: number
): Promise<ScopeInfo> => invoke("scope_add", { path, options, filesEst: filesEst ?? null });
export const scopeRemove = (scopeId: number, deleteIndex: boolean): Promise<void> =>
  invoke("scope_remove", { scopeId, deleteIndex });
export const scopeSetEnabled = (scopeId: number, enabled: boolean): Promise<void> =>
  invoke("scope_set_enabled", { scopeId, enabled });
export const scopeTrialScan = (path: string): Promise<{ filesEst: number; bytesEst: number; sample: string[] }> =>
  invoke("scope_trial_scan", { path });
export const scanScope = (scopeId: number): Promise<void> => invoke("cmd_scan", { scopeId });
export const recalibrateScope = (scopeId: number): Promise<void> =>
  invoke("cmd_recalibrate", { scopeId });
export const rebuildScope = (scopeId: number): Promise<void> => invoke("cmd_rebuild", { scopeId });

export const fulltextStart = (scopeId?: number): Promise<void> =>
  invoke("cmd_fulltext_start", { scopeId: scopeId ?? null });
export const ocrStart = (scopeId?: number): Promise<void> =>
  invoke("cmd_ocr_start", { scopeId: scopeId ?? null });

export const largeFiles = (minSize: number, limit: number, pathPrefix?: string): Promise<FileRow[]> =>
  invoke("large_files", { minSize, limit, pathPrefix: pathPrefix ?? null });
export const duplicatesStart = (minSize: number, keepHow: string): Promise<void> =>
  invoke("start_duplicates_cmd", { minSize, keepHow });
export const similarImages = (pathPrefix: string, threshold: number): Promise<{ files: FileRow[]; avgDistance: number }[]> =>
  invoke("similar_images", { pathPrefix, threshold });
export const organizeSuggest = (
  sourceDir: string,
  ruleType: string,
  targetDir: string
): Promise<PlanItem[]> => invoke("organize_suggest", { sourceDir, ruleType, targetDir });
export const organizeIssueToken = (
  sourceDir: string,
  ruleType: string,
  targetDir: string
): Promise<string> => invoke("organize_issue_token", { sourceDir, ruleType, targetDir });
export const organizeExecute = (
  sourceDir: string,
  ruleType: string,
  targetDir: string,
  token: string
): Promise<PlanItem[]> => invoke("organize_execute", { sourceDir, ruleType, targetDir, token });

export interface ScanProgress {
  progress: number;
  message: string | null;
  state: string;
}

export const scanStatus = (): Promise<ScanProgress | null> => invoke("scan_status");

export const jobsList = (limit?: number): Promise<JobInfo[]> => invoke("jobs_list", { limit: limit ?? 50 });
export const jobPause = (jobId: number): Promise<void> => invoke("job_pause", { jobId });
export const jobResume = (jobId: number): Promise<void> => invoke("job_resume", { jobId });
export const jobCancel = (jobId: number): Promise<void> => invoke("job_cancel", { jobId });
export const jobRetryFailed = (jobId: number): Promise<void> => invoke("job_retry_failed", { jobId });

export const diagnosticsHealth = (): Promise<IndexHealth> => invoke("diagnostics_health");
export const diagnosticsFailures = (): Promise<{ code: string | null; count: number }[]> =>
  invoke("diagnostics_failures");
export const diagnosticsExport = (target: string): Promise<string> =>
  invoke("diagnostics_export", { target });
export const diagnosticsIntegrity = (): Promise<string> => invoke("diagnostics_integrity");
export const logDirGet = (): Promise<string | null> => invoke("log_dir_get");

export const actionOpen = (path: string): Promise<string> => invoke("action_open", { path });
export const actionLocate = (path: string): Promise<string> => invoke("action_locate", { path });
/** 探测键入的路径是否存在：返回 "dir" / "file" / null。 */
export const pathProbe = (path: string): Promise<"dir" | "file" | null> => invoke("path_probe", { path });
export const actionCopyPath = (path: string, format: string): Promise<string> =>
  invoke("action_copy_path", { path, format });
export const actionCopyFiles = (paths: string[]): Promise<string> =>
  invoke("action_copy_files", { paths });
/** 删除目标：稳定 ID + 当前路径，供后端双重校验（产品底线 3）。 */
export interface DeleteTarget {
  fileId: number;
  path: string;
}

export const issueDeleteToken = (files: DeleteTarget[]): Promise<string> =>
  invoke("action_issue_delete_token", { files });
export const actionRecycle = (files: DeleteTarget[], token: string): Promise<PlanItem[]> =>
  invoke("action_recycle", { request: { files, token } });
export const actionDeletePermanent = (files: DeleteTarget[], token: string): Promise<PlanItem[]> =>
  invoke("action_delete_permanent", { request: { files, token } });
export const actionOpenWith = (path: string): Promise<string> => invoke("action_open_with", { path });
export const actionProperties = (path: string): Promise<string> => invoke("action_properties", { path });
export const actionOpenTerminal = (path: string): Promise<string> =>
  invoke("action_open_terminal", { path });
export const actionOpenVscode = (path: string): Promise<string> =>
  invoke("action_open_vscode", { path });
export const actionRename = (oldPath: string, newName: string): Promise<string> =>
  invoke("action_rename", { oldPath, newName });

/** 批量执行结果：聚合计数 + 逐项结果（底线 2：成功 / 跳过 / 失败可追溯）。 */
export interface BatchExecuteResult {
  ok: number;
  failed: number;
  skipped: number;
  /** 仅批量重命名返回，供一次性撤销。 */
  operationId?: number;
  items: PlanItem[];
}

export const batchRenamePreview = (paths: string[], rule: unknown): Promise<PlanItem[]> =>
  invoke("batch_rename_preview", { request: { paths, rule } });
export const batchRenameExecute = (items: PlanItem[]): Promise<BatchExecuteResult> =>
  invoke("batch_rename_execute", { items });
export const batchRenameUndo = (operationId: number): Promise<{ undone: number; conflicts: number }> =>
  invoke("batch_rename_undo", { operationId });
export const batchMovePreview = (paths: string[], targetDir: string): Promise<PlanItem[]> =>
  invoke("batch_move_preview", { paths, targetDir });
export const batchMoveExecute = (items: PlanItem[], conflict: string): Promise<BatchExecuteResult> =>
  invoke("batch_move_execute", { items, conflict });

export const previewGet = (path: string): Promise<PreviewPayload> => invoke("preview_get", { path });
export const favoritesList = (): Promise<FileRow[]> => invoke("favorites_list");
export const favoriteToggle = (fileId: number): Promise<boolean> =>
  invoke("favorite_toggle", { fileId });
export const tagsList = (): Promise<{ tagId: number; name: string; color: string }[]> =>
  invoke("tags_list");
export const tagCreate = (name: string, color: string): Promise<number> =>
  invoke("tag_create", { name, color });
export const tagDelete = (tagId: number): Promise<void> => invoke("tag_delete", { tagId });
export const fileTagsSet = (fileId: number, tagIds: number[]): Promise<void> =>
  invoke("file_tags_set", { fileId, tagIds });
export const recentsList = (): Promise<{ path: string; action: string; at: string }[]> =>
  invoke("recents_list");
export const fileByPath = (path: string): Promise<FileRow | null> => invoke("file_by_path", { path });

export const hotkeyApply = (hotkey: string): Promise<void> =>
  invoke("hotkey_apply", { newHotkey: hotkey });

export const autostartIsEnabled = (): Promise<boolean> => autostartIsEnabledRaw();
export const autostartEnable = (): Promise<void> => autostartEnableRaw();
export const autostartDisable = (): Promise<void> => autostartDisableRaw();
export interface StorageUsage {
  dataDir: string;
  dbBytes: number;
  indexBytes: number;
  /** 旧数据目录（默认目录 ≠ 当前目录时存在），供提示清理。 */
  legacyDir: string | null;
  /** 旧数据目录中可清理的冗余占用（数据库 + 索引）。 */
  legacyBytes: number;
}

export const storageUsage = (): Promise<StorageUsage> => invoke('storage_usage');
/** 清理旧数据目录中的冗余（数据库 + 索引），返回释放的字节数。 */
export const cleanupLegacyData = (): Promise<number> => invoke('cleanup_legacy_data');
export const setDataDir = (newDir: string): Promise<void> => invoke('set_data_dir', { newDir });
export const relaunchApp = (): Promise<void> => invoke('relaunch_app');

/** 显示或隐藏系统托盘图标（对应设置 ui.showTrayIcon）。 */
export const traySetVisible = (visible: boolean): Promise<void> =>
  invoke("tray_set_visible", { visible });
/** 完全退出应用：关闭窗口只隐藏到后台。 */
export const appExit = (): Promise<void> => invoke("app_exit");

export const updaterCheck = (): Promise<{ available: boolean; version: string | null }> =>
  invoke("updater_check");
export const updaterInstall = (): Promise<void> => invoke("updater_install");

export function formatBytes(n: number): string {
  if (n <= 0) return "—";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let v = n;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i += 1;
  }
  return `${v >= 100 ? Math.round(v) : v.toFixed(1)} ${units[i]}`;
}

export function formatTime(ms: number): string {
  if (!ms) return "—";
  const d = new Date(ms);
  const p = (x: number) => String(x).padStart(2, "0");
  return `${d.getFullYear()}/${p(d.getMonth() + 1)}/${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
}
