use tauri::Manager;

use crate::models::IndexHealth;

#[tauri::command]
pub fn diagnostics_health(
    store: tauri::State<'_, std::sync::Arc<crate::AppState>>,
) -> Result<IndexHealth, String> {
    store.db.with_conn(|c| {
        let total: i64 = c
            .query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        let ft_done: i64 = c
            .query_row("SELECT COUNT(*) FROM files WHERE fulltext_state='done'", [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        let ft_failed: i64 = c
            .query_row(
                "SELECT COUNT(*) FROM content_state WHERE status='failed'",
                [],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        let ocr_done: i64 = c
            .query_row("SELECT COUNT(*) FROM files WHERE ocr_state='done'", [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        let ocr_failed: i64 = c
            .query_row("SELECT COUNT(*) FROM files WHERE ocr_state='failed'", [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        let pending: i64 = c
            .query_row(
                "SELECT COUNT(*) FROM index_outbox WHERE state='pending'",
                [],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        let last_cp: i64 = c
            .query_row("SELECT COALESCE(MAX(updated_ms),0) FROM files", [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        let scopes = crate::scope::list_scopes(c)?;
        Ok(IndexHealth {
            total_files: total,
            fulltext_done: ft_done,
            fulltext_failed: ft_failed,
            ocr_done,
            ocr_failed,
            outbox_pending: pending,
            db_size_bytes: db_size(c),
            scopes,
            last_checkpoint_ms: last_cp,
        })
    })
}

fn db_size(c: &rusqlite::Connection) -> i64 {
    c.query_row("PRAGMA page_count", [], |r| r.get::<_, i64>(0))
        .map(|pages| {
            let page_size: i64 = c
                .query_row("PRAGMA page_size", [], |r| r.get(0))
                .unwrap_or(4096);
            pages * page_size
        })
        .unwrap_or(0)
}

/// 失败分类统计（任务与诊断页）。
#[tauri::command]
pub fn diagnostics_failures(
    store: tauri::State<'_, std::sync::Arc<crate::AppState>>,
) -> Result<Vec<serde_json::Value>, String> {
    store.db.with_conn(|c| {
        let mut stmt = c
            .prepare(
                "SELECT error_code, COUNT(*) FROM content_state WHERE status IN ('failed','skipped')
                 GROUP BY error_code ORDER BY COUNT(*) DESC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                Ok(serde_json::json!({"code": r.get::<_, Option<String>>(0)?, "count": r.get::<_, i64>(1)?}))
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    })
}

/// 导出诊断包：健康 + 失败统计 + 最近任务摘要（不含文件完整路径之外的敏感内容）。
#[tauri::command]
pub fn diagnostics_export(
    app: tauri::AppHandle,
    target: String,
) -> Result<String, String> {
    let state: tauri::State<crate::AppState> = app.state();
    let health = diagnostics_health_inner(&state)?;
    let report = serde_json::json!({
        "product": "FindFlow",
        "version": env!("CARGO_PKG_VERSION"),
        "exportedAt": crate::util::now_rfc3339(),
        "health": health,
    });
    std::fs::write(&target, serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    Ok(target)
}

fn diagnostics_health_inner(
    state: &crate::AppState,
) -> Result<IndexHealth, String> {
    store_health(&state.db)
}

fn store_health(db: &crate::settings::SettingsStore) -> Result<IndexHealth, String> {
    db.with_conn(|c| {
        let total: i64 = c.query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0)).map_err(|e| e.to_string())?;
        let ft_done: i64 = c.query_row("SELECT COUNT(*) FROM files WHERE fulltext_state='done'", [], |r| r.get(0)).map_err(|e| e.to_string())?;
        let ft_failed: i64 = c.query_row("SELECT COUNT(*) FROM content_state WHERE status='failed'", [], |r| r.get(0)).map_err(|e| e.to_string())?;
        let ocr_done: i64 = c.query_row("SELECT COUNT(*) FROM files WHERE ocr_state='done'", [], |r| r.get(0)).map_err(|e| e.to_string())?;
        let ocr_failed: i64 = c.query_row("SELECT COUNT(*) FROM files WHERE ocr_state='failed'", [], |r| r.get(0)).map_err(|e| e.to_string())?;
        let pending: i64 = c.query_row("SELECT COUNT(*) FROM index_outbox WHERE state='pending'", [], |r| r.get(0)).map_err(|e| e.to_string())?;
        let last_cp: i64 = c.query_row("SELECT COALESCE(MAX(updated_ms),0) FROM files", [], |r| r.get(0)).map_err(|e| e.to_string())?;
        let scopes = crate::scope::list_scopes(c)?;
        Ok(IndexHealth {
            total_files: total,
            fulltext_done: ft_done,
            fulltext_failed: ft_failed,
            ocr_done,
            ocr_failed,
            outbox_pending: pending,
            db_size_bytes: db_size(c),
            scopes,
            last_checkpoint_ms: last_cp,
        })
    })
}

/// 数据库轻量完整性检查（§12.2）。
#[tauri::command]
pub fn diagnostics_integrity(store: tauri::State<'_, std::sync::Arc<crate::AppState>>) -> Result<String, String> {
    store.db.with_conn(|c| {
        c.query_row("PRAGMA quick_check(1)", [], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn log_dir_get() -> Result<Option<String>, String> {
    Ok(crate::logger::log_dir().map(|p| p.to_string_lossy().to_string()))
}
