use tauri::Manager;

use crate::models::IndexHealth;

/// 全表聚合结果缓存：百万级文件时每次全表扫描需数百毫秒，设置页 3 秒轮询无需每次重算。
struct HeavyStats {
    at: std::time::Instant,
    total: i64,
    ft_done: i64,
    ft_failed: i64,
    ocr_done: i64,
    ocr_failed: i64,
    last_cp: i64,
}

static HEAVY: std::sync::Mutex<Option<HeavyStats>> = std::sync::Mutex::new(None);
const HEAVY_TTL: std::time::Duration = std::time::Duration::from_secs(15);

/// 范围开关 / 增删后让健康统计缓存失效，设置页的「已索引 N 个文件」才会立即变化。
pub fn invalidate_health_cache() {
    *HEAVY.lock().unwrap() = None;
}

#[tauri::command]
pub async fn diagnostics_health(
    store: tauri::State<'_, std::sync::Arc<crate::AppState>>,
) -> Result<IndexHealth, String> {
    compute_health(&store.stats_db)
}

/// 健康指标：统计专用连接执行，不阻塞界面搜索。
fn compute_health(db: &crate::settings::DbHandle) -> Result<IndexHealth, String> {
    db.with_conn(|c| {
        let mut heavy = HEAVY.lock().unwrap();
        if heavy.as_ref().map(|h| h.at.elapsed() >= HEAVY_TTL).unwrap_or(true) {
            // 单次扫描得出全部 files 聚合，替代原来的 4 次全表扫描
            let (total, ft_done, ocr_done, ocr_failed, last_cp) = c
                .query_row(
                    "SELECT COUNT(*),
                            COALESCE(SUM(fulltext_state='done'), 0),
                            COALESCE(SUM(ocr_state='done'), 0),
                            COALESCE(SUM(ocr_state='failed'), 0),
                            COALESCE(MAX(updated_ms), 0)
                     FROM files
                     WHERE scope_id IN (SELECT scope_id FROM scopes WHERE enabled=1)",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
                )
                .map_err(|e| e.to_string())?;
            let ft_failed: i64 = c
                .query_row(
                    "SELECT COUNT(*) FROM content_state WHERE status='failed'",
                    [],
                    |r| r.get(0),
                )
                .map_err(|e| e.to_string())?;
            *heavy = Some(HeavyStats {
                at: std::time::Instant::now(),
                total,
                ft_done,
                ft_failed,
                ocr_done,
                ocr_failed,
                last_cp,
            });
        }
        let h = heavy.as_ref().unwrap();
        let pending: i64 = c
            .query_row(
                // applied = 已写入 writer 但尚未提交，同样还没落盘，一并计入待处理
                "SELECT COUNT(*) FROM index_outbox WHERE state IN ('pending','applied')",
                [],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        let scopes = crate::scope::list_scopes(c)?;
        Ok(IndexHealth {
            total_files: h.total,
            fulltext_done: h.ft_done,
            fulltext_failed: h.ft_failed,
            ocr_done: h.ocr_done,
            ocr_failed: h.ocr_failed,
            outbox_pending: pending,
            db_size_bytes: db_size(c),
            scopes,
            last_checkpoint_ms: h.last_cp,
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
pub async fn diagnostics_failures(
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
pub async fn diagnostics_export(
    app: tauri::AppHandle,
    target: String,
) -> Result<String, String> {
    let state: tauri::State<std::sync::Arc<crate::AppState>> = app.state();
    let health = compute_health(&state.stats_db)?;
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

/// 数据库轻量完整性检查（§12.2）。
#[tauri::command]
pub async fn diagnostics_integrity(store: tauri::State<'_, std::sync::Arc<crate::AppState>>) -> Result<String, String> {
    // 完整性检查需遍历整个库，走统计连接避免阻塞界面
    store.stats_db.with_conn(|c| {
        c.query_row("PRAGMA quick_check(1)", [], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub async fn log_dir_get() -> Result<Option<String>, String> {
    Ok(crate::logger::log_dir().map(|p| p.to_string_lossy().to_string()))
}
