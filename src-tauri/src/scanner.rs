use std::path::Path;
use std::sync::Arc;

use rusqlite::{params, Connection};

use crate::jobs::JobContext;
use crate::scope::{scope_options, ScopeOptions};
use crate::settings::SettingsStore;
use crate::util;

/// 读取单个路径的元数据行；失败时返回 None（跳过并计数，不中断任务）。
pub fn stat_entry(path: &Path, scope_id: i64, opts: &ScopeOptions) -> Option<FileUpsert> {
    let meta = std::fs::metadata(path).ok()?;
    let attrs = {
        use std::os::windows::fs::MetadataExt;
        meta.file_attributes()
    };
    let name = path.file_name()?.to_string_lossy().to_string();
    if !opts.include_hidden && util::is_hidden(attrs) {
        return None;
    }
    let is_dir = meta.is_dir();
    let ext = if is_dir {
        String::new()
    } else {
        path.extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default()
    };
    if !is_dir && !opts.exclude_exts.is_empty() && crate::scope::segment_excluded(&name, opts) {
        return None;
    }
    let modified_ms = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let created_ms = meta
        .created()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let (serial, native_id) = if is_dir {
        (None, None)
    } else {
        crate::identity::native_identity(path)
            .map(|(s, id)| (Some(s as i64), Some(id as i64)))
            .unwrap_or((None, None))
    };
    Some(FileUpsert {
        scope_id,
        name,
        path: util::normalize_path(path),
        kind: util::kind_from(&ext, is_dir).to_string(),
        ext,
        size: if is_dir { 0 } else { meta.len() as i64 },
        created_ms,
        modified_ms,
        attributes: attrs as i64,
        is_dir,
        native_serial: serial,
        native_file_id: native_id,
        parent_path: path
            .parent()
            .map(|p| util::normalize_path(p))
            .unwrap_or_default(),
    })
}

#[derive(Debug, Clone)]
pub struct FileUpsert {
    pub scope_id: i64,
    pub name: String,
    pub path: String,
    pub ext: String,
    pub kind: String,
    pub size: i64,
    pub created_ms: i64,
    pub modified_ms: i64,
    pub attributes: i64,
    pub is_dir: bool,
    pub native_serial: Option<i64>,
    pub native_file_id: Option<i64>,
    pub parent_path: String,
}

/// 批量写入文件行，并为每行登记待索引事件（outbox，设计文档 §7.3）。
pub fn upsert_batch(c: &Connection, items: &[FileUpsert]) -> Result<(), String> {
    for item in items {
        c.execute(
            "INSERT INTO files(scope_id, native_serial, native_file_id, parent_path, name, path, ext, kind,
                               size, created_ms, modified_ms, attributes, is_dir, updated_ms)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)
             ON CONFLICT(path) DO UPDATE SET
               native_serial=excluded.native_serial,
               native_file_id=excluded.native_file_id,
               parent_path=excluded.parent_path,
               name=excluded.name,
               ext=excluded.ext,
               kind=excluded.kind,
               size=excluded.size,
               created_ms=excluded.created_ms,
               modified_ms=excluded.modified_ms,
               attributes=excluded.attributes,
               is_dir=excluded.is_dir,
               updated_ms=excluded.updated_ms,
               fulltext_state=CASE WHEN files.modified_ms<>excluded.modified_ms OR files.size<>excluded.size
                                   THEN 'pending' ELSE files.fulltext_state END",
            params![
                item.scope_id,
                item.native_serial,
                item.native_file_id,
                item.parent_path,
                item.name,
                item.path,
                item.ext,
                item.kind,
                item.size,
                item.created_ms,
                item.modified_ms,
                item.attributes,
                item.is_dir as i64,
                util::now_ms()
            ],
        )
        .map_err(|e| e.to_string())?;
        let fid: i64 = c
            .query_row("SELECT file_id FROM files WHERE path=?1", [&item.path], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        c.execute(
            "INSERT INTO index_outbox(file_id, operation, created_ms) VALUES (?1,'upsert',?2)",
            params![fid, util::now_ms()],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn delete_by_path(c: &Connection, path: &str) -> Result<(), String> {
    let fid: Option<i64> = c
        .query_row("SELECT file_id FROM files WHERE path=?1", [path], |r| r.get(0))
        .ok();
    if let Some(fid) = fid {
        c.execute("DELETE FROM files WHERE file_id=?1", [fid])
            .map_err(|e| e.to_string())?;
        c.execute(
            "INSERT INTO index_outbox(file_id, operation, created_ms) VALUES (?1,'delete',?2)",
            params![fid, util::now_ms()],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// 遍历目录树：受保护目录、隐藏目录、排除规则整树跳过。
///
/// - `count_only = true`：只统计将入库的条目数（快速，不做文件标识/入库）。
/// - `count_only = false`：收集元数据并按批回调写出。
#[allow(clippy::too_many_arguments)]
fn walk_inner(
    dir: &Path,
    depth: usize,
    excluded: bool,
    opts: &ScopeOptions,
    count_only: bool,
    batch: &mut Vec<FileUpsert>,
    on_batch: &mut impl FnMut(Vec<FileUpsert>),
    errors: &mut u64,
    seen: &mut u64,
    total: &mut u64,
    ctx: Option<&JobContext>,
) {
    let read = match std::fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => {
            *errors += 1;
            return;
        }
    };
    for entry in read.flatten() {
        if let Some(ctx) = ctx {
            ctx.wait_if_paused();
            if ctx.cancelled() {
                return;
            }
            *seen += 1;
            if *seen % 20000 == 0 {
                // 遍历阶段持续给出反馈，避免看起来像卡死
                ctx.progress(0.0, &format!("正在遍历，已发现 {} 项", seen));
            }
        }
        let path = entry.path();
        let Ok(meta) = std::fs::metadata(&path) else {
            *errors += 1;
            continue;
        };
        let attrs = {
            use std::os::windows::fs::MetadataExt;
            meta.file_attributes()
        };
        let name = entry.file_name().to_string_lossy().to_string();
        let hidden = util::is_hidden(attrs);
        let child_excluded = excluded
            || (!opts.include_hidden && hidden)
            || crate::scope::segment_excluded(&name, opts);
        if meta.is_dir() {
            if depth > 0
                && (child_excluded
                    || crate::scope::is_protected(&util::normalize_path(&path))
                    || name.starts_with('$'))
            {
                continue; // 整树跳过
            }
            if depth > 0 && !child_excluded {
                if count_only {
                    *total += 1;
                } else {
                    let mut u = stat_entry(&path, 0, opts);
                    if let Some(uu) = &mut u {
                        uu.kind = "folder".into();
                        uu.is_dir = true;
                        uu.ext = String::new();
                        uu.size = 0;
                    }
                    if let Some(uu) = u {
                        batch.push(uu);
                    }
                }
            }
            walk_inner(
                &path,
                depth + 1,
                child_excluded,
                opts,
                count_only,
                batch,
                on_batch,
                errors,
                seen,
                total,
                ctx,
            );
        } else if !child_excluded {
            if count_only {
                *total += 1;
            } else if let Some(mut u) = stat_entry(&path, 0, opts) {
                u.scope_id = 0;
                batch.push(u);
            }
        }
        if !count_only && batch.len() >= 500 {
            on_batch(std::mem::take(batch));
        }
    }
}

/// 阶段一：快速统计范围总数（仅枚举目录项，不打开文件取标识）。
fn count_dir(root: &Path, opts: &ScopeOptions, ctx: Option<&JobContext>) -> (u64, u64) {
    let mut errors = 0u64;
    let mut seen = 0u64;
    let mut total = 0u64;
    let mut batch: Vec<FileUpsert> = Vec::with_capacity(500);
    let mut on_batch = |_: Vec<FileUpsert>| {};
    walk_inner(
        root,
        0,
        false,
        opts,
        true,
        &mut batch,
        &mut on_batch,
        &mut errors,
        &mut seen,
        &mut total,
        ctx,
    );
    if !batch.is_empty() {
        on_batch(batch);
    }
    (total, errors)
}

/// 阶段二：流式收集元数据，每凑满一批即回调写出。
fn walk_dir(
    root: &Path,
    opts: &ScopeOptions,
    ctx: Option<&JobContext>,
    mut on_batch: impl FnMut(Vec<FileUpsert>),
) -> u64 {
    let mut errors = 0u64;
    let mut seen = 0u64;
    let mut total = 0u64;
    let mut batch: Vec<FileUpsert> = Vec::with_capacity(500);
    walk_inner(
        root,
        0,
        false,
        opts,
        false,
        &mut batch,
        &mut on_batch,
        &mut errors,
        &mut seen,
        &mut total,
        ctx,
    );
    if !batch.is_empty() {
        on_batch(batch);
    }
    errors
}

pub fn start_scan_job(state: &crate::AppState, scope_id: i64) -> Result<(), String> {
    let db = state.engine.db.clone();
    let (root, opts_json) = db.with_conn(|c| {
        c.query_row(
            "SELECT path, options_json FROM scopes WHERE scope_id=?1",
            [scope_id],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        )
        .map_err(|e| e.to_string())
    })?;
    let opts = scope_options(&opts_json);
    let db_for_body = db.clone();
    state.engine.start(
        "initial_scan",
        serde_json::json!({ "scopeId": scope_id, "path": root }),
        move |ctx| run_initial_scan(ctx, db_for_body, scope_id, root, opts),
    )?;
    state.db.with_conn(|c| {
        c.execute("UPDATE scopes SET state='scanning' WHERE scope_id=?1", [scope_id])
            .ok();
    });
    Ok(())
}

fn run_initial_scan(
    ctx: JobContext,
    db: Arc<SettingsStore>,
    scope_id: i64,
    root: String,
    opts: ScopeOptions,
) {
    // 阶段一：快速统计总数（准确进度的分母；仅枚举目录项，远快于正式扫描）
    ctx.progress(0.0, "正在统计文件数量…");
    let (total, count_errors) = count_dir(Path::new(&root), &opts, Some(&ctx));
    if ctx.cancelled() {
        ctx.finish_cancelled();
        db.with_conn(|c| {
            c.execute("UPDATE scopes SET state='idle' WHERE scope_id=?1", [scope_id])
                .ok();
        });
        return;
    }
    if total == 0 {
        db.with_conn(|c| {
            c.execute(
                "UPDATE scopes SET state='idle', baseline_at=?2, last_scan_ms=?3 WHERE scope_id=?1",
                params![scope_id, util::now_rfc3339(), util::now_ms()],
            )
            .ok();
        });
        ctx.finish_ok(Some(
            serde_json::json!({ "scopeId": scope_id, "indexed": 0, "errors": count_errors, "path": root })
                .to_string(),
        ));
        return;
    }

    // 阶段二：流式扫描，进度 = 已索引 / 准确总数，百分比稳定不跳
    let mut done = 0f64;
    let mut failed = count_errors;
    let walk_errors = walk_dir(Path::new(&root), &opts, Some(&ctx), |mut batch| {
        ctx.wait_if_paused();
        if ctx.cancelled() {
            return;
        }
        for u in &mut batch {
            u.scope_id = scope_id;
        }
        if let Err(e) = db.with_conn(|c| {
            c.execute("BEGIN IMMEDIATE", []).ok();
            if let Err(e) = upsert_batch(c, &batch) {
                c.execute("ROLLBACK", []).ok();
                return Err(e);
            }
            c.execute("COMMIT", []).map_err(|e| e.to_string())?;
            Ok::<(), String>(())
        }) {
            failed += batch.len() as u64;
            tracing::warn!("扫描批次写入失败: {e}");
        }
        done += batch.len() as f64;
        ctx.progress(
            (done / total as f64).min(0.99),
            &format!("已索引 {} / {} 项", done as u64, total),
        );
    });
    failed += walk_errors;

    db.with_conn(|c| {
        c.execute(
            "UPDATE scopes SET state='idle', baseline_at=?2, last_scan_ms=?3 WHERE scope_id=?1",
            params![scope_id, util::now_rfc3339(), util::now_ms()],
        )
        .ok();
    });
    ctx.finish_ok(Some(
        serde_json::json!({ "scopeId": scope_id, "indexed": done as u64, "errors": failed, "path": root })
            .to_string(),
    ));
}

/// 校准扫描：磁盘与索引做差集修正，处理监听离线期间的变化（FR04）。
pub fn start_recalibrate_job(state: &crate::AppState, scope_id: i64) -> Result<(), String> {
    let db = state.engine.db.clone();
    let (root, opts_json) = db.with_conn(|c| {
        c.query_row(
            "SELECT path, options_json FROM scopes WHERE scope_id=?1",
            [scope_id],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        )
        .map_err(|e| e.to_string())
    })?;
    let opts = scope_options(&opts_json);
    let db_for_body = db.clone();
    state.engine.start(
        "recalibrate",
        serde_json::json!({ "scopeId": scope_id }),
        move |ctx| run_recalibrate(ctx, db_for_body, scope_id, root, opts),
    )?;
    Ok(())
}

fn run_recalibrate(
    ctx: JobContext,
    db: Arc<SettingsStore>,
    scope_id: i64,
    root: String,
    opts: ScopeOptions,
) {
    let mut disk_paths = std::collections::HashSet::new();
    let mut rec_done = 0f64;
    let errors = walk_dir(Path::new(&root), &opts, Some(&ctx), |batch| {
        if ctx.cancelled() {
            return;
        }
        let mut fixed: Vec<FileUpsert> = Vec::with_capacity(batch.len());
        for mut u in batch {
            u.scope_id = scope_id;
            disk_paths.insert(u.path.clone());
            fixed.push(u);
        }
        rec_done += fixed.len() as f64;
        let _ = db.with_conn(|c| {
            c.execute("BEGIN IMMEDIATE", []).ok();
            let _ = upsert_batch(c, &fixed);
            c.execute("COMMIT", []).ok();
        });
        ctx.progress(0.0, &format!("校准对比中，已核对 {} 项", rec_done as u64));
    });

    let all_paths: Vec<String> = db
        .with_conn(|c| {
            let mut stmt = c
                .prepare("SELECT path FROM files WHERE scope_id=?1")
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map([scope_id], |r| r.get::<_, String>(0))
                .map_err(|e| e.to_string())?;
            let all: Vec<String> = rows.filter_map(|r| r.ok()).collect();
            Ok::<_, String>(all)
        })
        .unwrap_or_default();
    let removed: Vec<String> = all_paths
        .into_iter()
        .filter(|p| !disk_paths.contains(p))
        .collect();
    let removed_count = removed.len();
    for chunk in removed.chunks(300) {
        let _ = db.with_conn(|c| {
            c.execute("BEGIN IMMEDIATE", []).ok();
            for p in chunk {
                let _ = delete_by_path(c, p);
            }
            c.execute("COMMIT", []).ok();
        });
    }
    db.with_conn(|c| {
        c.execute(
            "UPDATE scopes SET baseline_at=?2, last_scan_ms=?3, state='idle' WHERE scope_id=?1",
            params![scope_id, util::now_rfc3339(), util::now_ms()],
        )
        .ok();
    });
    ctx.finish_ok(Some(
        serde_json::json!({ "scopeId": scope_id, "removed": removed_count, "errors": errors }).to_string(),
    ));
}

/// 重建范围：清空该范围的索引后重扫（FR18）。
pub fn start_rebuild_job(state: &crate::AppState, scope_id: i64) -> Result<(), String> {
    state
        .db
        .with_conn(|c| {
            c.execute("DELETE FROM files WHERE scope_id=?1", [scope_id])
                .map_err(|e| e.to_string())?;
            Ok::<(), String>(())
        })
        .ok();
    start_scan_job(state, scope_id)
}

#[tauri::command]
pub fn cmd_scan(state: tauri::State<'_, crate::AppState>, scope_id: i64) -> Result<(), String> {
    start_scan_job(&state, scope_id)
}

#[tauri::command]
pub fn cmd_recalibrate(state: tauri::State<'_, crate::AppState>, scope_id: i64) -> Result<(), String> {
    start_recalibrate_job(&state, scope_id)
}

#[tauri::command]
pub fn cmd_rebuild(state: tauri::State<'_, crate::AppState>, scope_id: i64) -> Result<(), String> {
    start_rebuild_job(&state, scope_id)
}
