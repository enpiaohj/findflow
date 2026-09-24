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

pub fn is_protected(path: &str) -> bool {
    path.split('\\')
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
    let mut stmt = c
        .prepare(
            "SELECT s.scope_id, s.path, s.enabled, s.state, s.baseline_at,
                    (SELECT COUNT(*) FROM files f WHERE f.scope_id = s.scope_id)
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
                files_count: r.get(4)?,
                baseline_at: r.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn scopes_list(store: tauri::State<'_, crate::AppState>) -> Result<Vec<crate::models::ScopeInfo>, String> {
    store.db.with_conn(list_scopes)
}

#[tauri::command]
pub fn scope_add(
    store: tauri::State<'_, crate::AppState>,
    path: String,
    options: Option<serde_json::Value>,
) -> Result<crate::models::ScopeInfo, String> {
    let path = path.trim_end_matches('\\').to_string();
    if path.len() < 3 || !path.chars().nth(1).map(|c| c == ':').unwrap_or(false) {
        return Err("请输入本地磁盘或目录的绝对路径（如 D:\\项目资料）".into());
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
                "INSERT OR IGNORE INTO scopes(path, options_json, added_at) VALUES (?1, ?2, ?3)",
                rusqlite::params![path, opts.to_string(), util::now_rfc3339()],
            )
                .map_err(|e| e.to_string())?;
            Ok::<i64, String>(c.last_insert_rowid())
        })?;
    store.watch_scope(scope_id, &path)?;
    let _ = crate::scanner::start_scan_job(&store, scope_id);
    Ok(crate::models::ScopeInfo {
        scope_id,
        path,
        enabled: true,
        state: "scanning".into(),
        files_count: 0,
        baseline_at: None,
    })
}

#[tauri::command]
pub fn scope_remove(
    store: tauri::State<'_, crate::AppState>,
    scope_id: i64,
    delete_index: bool,
) -> Result<(), String> {
    store.db.with_conn(|c| -> Result<(), String> {
        if delete_index {
            c.execute("DELETE FROM files WHERE scope_id=?1", [scope_id]).map_err(|e| e.to_string())?;
            c.execute("DELETE FROM scopes WHERE scope_id=?1", [scope_id]).map_err(|e| e.to_string())?;
        } else {
            c.execute("UPDATE scopes SET enabled=0, state='offline' WHERE scope_id=?1", [scope_id]).map_err(|e| e.to_string())?;
        }
        Ok(())
    })?;
    store.unwatch_scope(scope_id);
    Ok(())
}

#[tauri::command]
pub fn scope_set_enabled(
    store: tauri::State<'_, crate::AppState>,
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
    if !enabled {
        store.unwatch_scope(scope_id);
    } else {
        let path: Result<String, _> = store.db.with_conn(|c| {
            c.query_row("SELECT path FROM scopes WHERE scope_id=?1", [scope_id], |r| r.get(0))
        });
        if let Ok(p) = path {
            let _ = store.watch_scope(scope_id, &p);
        }
    }
    Ok(())
}

#[tauri::command]
pub fn scope_update_options(
    store: tauri::State<'_, crate::AppState>,
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
pub fn scope_trial_scan(path: String) -> Result<crate::models::TrialScanResult, String> {
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
    fn protected_dirs_rejected() {
        assert!(is_protected("C:\\Windows\\System32"));
        assert!(is_protected("D:\\Program Files\\App"));
        assert!(!is_protected("D:\\项目资料"));
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
