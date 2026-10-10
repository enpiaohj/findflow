use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::util;

/// 系统与程序目录默认受保护，任何范围配置都不得索引（FR13、§10.2）。
const PROTECTED_DIRS: &[&str] = &[
    "windows",
    "program files",
    "program files (x86)",
    "programdata",
    "$recycle.bin",
    "system volume information",
    "appdata",
    "$windows.~bt",
];

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ScopeOptions {
    pub exclude_dirs: Vec<String>,
    pub exclude_exts: Vec<String>,
    #[serde(default)]
    pub include_hidden: bool,
}

/// 是否为 UNC 网络路径（`\\server\share\...`）。
pub fn is_unc_path(p: &str) -> bool {
    let b = p.trim().as_bytes();
    b.len() >= 2 && b[0] == b'\\' && b[1] == b'\\'
}

/// 盘符根目录（D: / D:\\）归一化为 D:\\，其余目录去掉尾部分隔符。
/// UNC 路径（`\\server\share`）保留双前缀，只去尾部分隔符。
pub fn normalize_scope_path(p: &str) -> String {
    let trimmed = p.trim();
    if is_unc_path(trimmed) {
        // 共享根 `\\server\share\` 去尾部分隔符后为 `\\server\share`
        return trimmed.trim_end_matches('\\').to_string();
    }
    let bytes = trimmed.as_bytes();
    if bytes.len() == 2 && bytes[1] == b':' {
        // 盘符统一大写，显示一致
        format!("{}{}\\", trimmed[..1].to_ascii_uppercase(), &trimmed[1..])
    } else if bytes.len() == 3 && bytes[1] == b':' && trimmed.ends_with('\\') {
        trimmed.to_string()
    } else {
        trimmed.trim_end_matches('\\').to_string()
    }
}

/// 路径是否可作为索引范围：本地盘符绝对路径（`D:\...`）或 UNC 网络路径（`\\server\share\...`）。
pub fn is_indexable_scope_path(p: &str) -> bool {
    if is_unc_path(p) {
        // 至少要有 `\\server\share` 三段
        return p.trim_matches('\\').split('\\').count() >= 2;
    }
    p.len() >= 3 && p.chars().nth(1).map(|c| c == ':').unwrap_or(false)
}

/// 路径是否命中受保护目录。先展开 8.3 短名（`C:\PROGRA~1` → `C:\Program Files`），
/// 否则短名写法可绕过按段比对（中危 #5）。
pub fn is_protected(path: &str) -> bool {
    let long = util::to_long_path(path);
    long.split('\\')
        .any(|seg| PROTECTED_DIRS.contains(&seg.to_ascii_lowercase().as_str()))
}

pub fn scope_options(options_json: &str) -> ScopeOptions {
    serde_json::from_str(options_json).unwrap_or_default()
}

/// 判断单个路径段是否命中排除规则。
pub fn segment_excluded(seg: &str, opts: &ScopeOptions) -> bool {
    let lower = seg.to_ascii_lowercase();
    if opts
        .exclude_dirs
        .iter()
        .any(|d| d.to_ascii_lowercase() == lower)
    {
        return true;
    }
    if let Some((_, ext)) = lower.rsplit_once('.') {
        opts.exclude_exts.iter().any(|e| e.to_ascii_lowercase() == ext)
    } else {
        false
    }
}

pub fn list_scopes(c: &Connection) -> Result<Vec<crate::models::ScopeInfo>, String> {
    // 状态自愈：没有运行中的扫描任务却停留在 scanning，说明上次异常中断
    c.execute(
        "UPDATE scopes SET state='idle'
         WHERE state='scanning'
         AND NOT EXISTS (
           SELECT 1 FROM jobs
           WHERE kind='initial_scan' AND state IN ('waiting','running','paused')
         )",
        [],
    )
    .map_err(|e| e.to_string())?;
    let mut stmt = c
        .prepare(
            "SELECT s.scope_id, s.path, s.enabled, s.state, s.baseline_at,
                    (SELECT COUNT(*) FROM files f WHERE f.scope_id = s.scope_id),
                    s.options_json
             FROM scopes s ORDER BY s.scope_id",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(crate::models::ScopeInfo {
                scope_id: r.get(0)?,
                path: r.get(1)?,
                enabled: r.get::<_, i64>(2)? != 0,
                state: r.get(3)?,
                // SELECT 列序：baseline_at 第 5 列（可空），files_count 是子查询第 6 列
                baseline_at: r.get(4)?,
                files_count: r.get(5)?,
                options_json: r.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn scopes_list(store: tauri::State<'_, std::sync::Arc<crate::AppState>>) -> Result<Vec<crate::models::ScopeInfo>, String> {
    // 各范围文件计数需扫描索引，走统计连接避免阻塞界面搜索
    store.stats_db.with_conn(list_scopes)
}

#[tauri::command]
pub async fn scope_add(
    store: tauri::State<'_, std::sync::Arc<crate::AppState>>,
    path: String,
    options: Option<serde_json::Value>,
    files_est: Option<i64>,
) -> Result<crate::models::ScopeInfo, String> {
    let path = normalize_scope_path(&path);
    if !is_indexable_scope_path(&path) {
        return Err("请输入本地磁盘路径（如 D:\\项目资料）或网络共享路径（如 \\\\服务器\\共享）".into());
    }
    if is_protected(&path) {
        return Err("系统与程序目录受保护，不能加入索引".into());
    }
    if !std::path::Path::new(&path).is_dir() {
        return Err("目录不存在或不可访问".into());
    }
    let opts = options.unwrap_or(serde_json::json!({}));
    let scope_id: i64 = store
        .db
        .with_conn(|c| {
            c.execute(
                "INSERT OR IGNORE INTO scopes(path, options_json, files_est, added_at) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![path, opts.to_string(), files_est, util::now_rfc3339()],
            )
                .map_err(|e| e.to_string())?;
            Ok::<i64, String>(c.last_insert_rowid())
        })?;
    // 监听失败不阻断添加：范围已入库，扫描照常；仅失去实时增量（可重新校准补齐）。
    if let Err(e) = store.watch_scope(scope_id, &path) {
        tracing::warn!("范围已加入但实时监听未启用（{path}）：{e}");
    }
    let _ = crate::scanner::start_scan_job(&store, scope_id);
    Ok(crate::models::ScopeInfo {
        scope_id,
        path,
        enabled: true,
        state: "scanning".into(),
        files_count: 0,
        baseline_at: None,
        options_json: opts.to_string(),
    })
}

#[tauri::command]
pub async fn scope_remove(
    store: tauri::State<'_, std::sync::Arc<crate::AppState>>,
    scope_id: i64,
    delete_index: bool,
) -> Result<(), String> {
    store.db.with_conn(|c| -> Result<(), String> {
        c.execute("UPDATE scopes SET enabled=0, state='offline' WHERE scope_id=?1", [scope_id]).map_err(|e| e.to_string())?;
        Ok(())
    })?;
    crate::search::invalidate_total_cache();
    crate::diagnostics::invalidate_health_cache();
    store.unwatch_scope(scope_id);
    if delete_index {
        // 百万级行的删除放到后台分块执行（同步删除索引文档），界面立即返回
        let bg = store.bg_db.clone();
        std::thread::Builder::new()
            .name("scope-purge".into())
            .spawn(move || {
                crate::scanner::purge_scope(&bg, scope_id);
                bg.with_conn(|c| {
                    c.execute("DELETE FROM scopes WHERE scope_id=?1", [scope_id]).ok();
                });
            })
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn scope_set_enabled(
    store: tauri::State<'_, std::sync::Arc<crate::AppState>>,
    scope_id: i64,
    enabled: bool,
) -> Result<(), String> {
    store
        .db
        .with_conn(|c| {
            c.execute(
                "UPDATE scopes SET enabled=?2 WHERE scope_id=?1",
                rusqlite::params![scope_id, enabled as i64],
            )
                .map_err(|e| e.to_string())?;
            Ok::<(), String>(())
        })?;
    crate::search::invalidate_total_cache();
    crate::diagnostics::invalidate_health_cache();
    if !enabled {
        store.unwatch_scope(scope_id);
    } else {
        let path: Result<String, _> = store.db.with_conn(|c| {
            c.query_row("SELECT path FROM scopes WHERE scope_id=?1", [scope_id], |r| r.get(0))
        });
        if let Ok(p) = path {
            let _ = store.watch_scope(scope_id, &p);
        }
        // 关闭期间没有监听，磁盘上的变化要补一次校准才会反映到索引里
        if let Err(e) = crate::scanner::start_recalibrate_job(&store, Some(scope_id)) {
            tracing::warn!("重新启用范围后校准失败: {e}");
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn scope_update_options(
    store: tauri::State<'_, std::sync::Arc<crate::AppState>>,
    scope_id: i64,
    options: serde_json::Value,
) -> Result<(), String> {
    store.db.with_conn(|c| -> Result<(), String> {
        c.execute(
            "UPDATE scopes SET options_json=?2 WHERE scope_id=?1",
            rusqlite::params![scope_id, options.to_string()],
        )
            .map_err(|e| e.to_string())?;
        Ok(())
    })
}

#[tauri::command]
pub async fn scope_trial_scan(path: String) -> Result<crate::models::TrialScanResult, String> {
    let root = std::path::PathBuf::from(&path);
    if !root.is_dir() {
        return Err("目录不存在或不可访问".into());
    }
    let mut files_est = 0i64;
    let mut bytes_est = 0i64;
    let mut sample = Vec::new();
    for entry in walkdir::WalkDir::new(&root)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .take(20_000)
    {
        if !entry.file_type().is_dir() {
            files_est += 1;
            if sample.len() < 8 && sample.len() % 300 == 0 {
                sample.push(entry.path().to_string_lossy().replace('/', "\\"));
            }
            if let Ok(meta) = entry.metadata() {
                bytes_est += meta.len() as i64;
            }
        }
    }
    Ok(crate::models::TrialScanResult {
        files_est,
        bytes_est,
        sample,
        accessible: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drive_root_paths_normalized() {
        assert_eq!(normalize_scope_path("D:"), "D:\\");
        assert_eq!(normalize_scope_path("D:\\"), "D:\\");
        assert_eq!(normalize_scope_path(" d: "), "D:\\");
        assert_eq!(normalize_scope_path("D:\\项目资料\\"), "D:\\项目资料");
        assert!(normalize_scope_path("D:").len() >= 3);
    }

    #[test]
    fn protected_dirs_rejected() {
        assert!(is_protected("C:\\Windows\\System32"));
        assert!(is_protected("D:\\Program Files\\App"));
        assert!(!is_protected("D:\\项目资料"));
    }

    #[test]
    fn unc_paths_recognized_and_normalized() {
        // UNC 前缀识别
        assert!(is_unc_path("\\\\fileserver\\share\\项目资料"));
        assert!(is_unc_path("\\\\server\\share"));
        assert!(!is_unc_path("D:\\项目资料"));
        assert!(!is_unc_path("D:\\"));
        assert!(!is_unc_path("\\单反斜杠"));

        // 归一化：保留双前缀，只去尾部分隔符
        assert_eq!(
            normalize_scope_path("\\\\fileserver\\share\\项目资料\\"),
            "\\\\fileserver\\share\\项目资料"
        );
        assert_eq!(
            normalize_scope_path("\\\\server\\share"),
            "\\\\server\\share"
        );
        // 盘符路径行为不变
        assert_eq!(normalize_scope_path("D:\\项目资料\\"), "D:\\项目资料");
        assert_eq!(normalize_scope_path("D:"), "D:\\");
    }

    #[test]
    fn indexable_scope_path_accepts_drives_and_unc() {
        // 本地盘符
        assert!(is_indexable_scope_path("D:\\"));
        assert!(is_indexable_scope_path("D:\\项目资料"));
        // UNC 需至少 \\server\share 两段
        assert!(is_indexable_scope_path("\\\\fileserver\\share\\项目资料"));
        assert!(is_indexable_scope_path("\\\\server\\share"));
        assert!(!is_indexable_scope_path("\\\\server"));
        assert!(!is_indexable_scope_path("\\\\"));
        // 相对路径与裸盘符仍拒绝
        assert!(!is_indexable_scope_path("项目资料"));
        assert!(!is_indexable_scope_path("D:"));
        assert!(!is_indexable_scope_path(""));
    }

    #[test]
    fn segment_exclusion_rules() {
        let opts = ScopeOptions {
            exclude_dirs: vec!["node_modules".into()],
            exclude_exts: vec!["tmp".into()],
            include_hidden: false,
        };
        assert!(segment_excluded("node_modules", &opts));
        assert!(segment_excluded("a.tmp", &opts));
        assert!(!segment_excluded("src", &opts));
    }
}
