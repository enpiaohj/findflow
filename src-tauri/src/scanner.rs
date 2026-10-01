use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::Arc;

use rusqlite::{params, Connection};

use crate::jobs::JobContext;
use crate::scope::{scope_options, ScopeOptions};
use crate::settings::DbHandle;
use crate::util;

/// 读取单个路径的元数据行；失败时返回 None（跳过并计数，不中断任务）。
pub fn stat_entry(path: &Path, scope_id: i64, opts: &ScopeOptions) -> Option<FileUpsert> {
    let meta = std::fs::metadata(path).ok()?;
    stat_with_meta(path, &meta, scope_id, opts)
}

/// 用已取得的元数据组装行（遍历时复用目录枚举自带的元数据，省去一次系统调用）。
fn stat_with_meta(
    path: &Path,
    meta: &std::fs::Metadata,
    scope_id: i64,
    opts: &ScopeOptions,
) -> Option<FileUpsert> {
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
            .map(util::normalize_path)
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

/// 批量写入文件行，并为需要重建索引文档的行登记待索引事件（outbox，设计文档 §7.3）。
///
/// 索引文档只含文件名与内容：新增行或文件名变化才入队；内容变化由全文任务自行入队。
/// 元数据未变化的行不写库（校准时绝大多数行不变，避免百万级无效写入与 WAL 膨胀）。
pub fn upsert_batch(c: &Connection, items: &[FileUpsert]) -> Result<(), String> {
    for item in items {
        let existing: Option<(i64, String)> = c
            .prepare_cached("SELECT file_id, name FROM files WHERE path=?1")
            .and_then(|mut s| s.query_row([&item.path], |r| Ok((r.get(0)?, r.get(1)?))))
            .ok();
        c.prepare_cached(
            "INSERT INTO files(scope_id, native_serial, native_file_id, parent_path, name, path, ext, kind,
                               size, created_ms, modified_ms, attributes, is_dir, updated_ms)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)
             ON CONFLICT(path) DO UPDATE SET
               native_serial=COALESCE(excluded.native_serial, files.native_serial),
               native_file_id=COALESCE(excluded.native_file_id, files.native_file_id),
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
                                   THEN 'pending' ELSE files.fulltext_state END
             WHERE files.name<>excluded.name
                OR files.size<>excluded.size
                OR files.modified_ms<>excluded.modified_ms
                OR files.created_ms<>excluded.created_ms
                OR files.attributes<>excluded.attributes
                OR files.is_dir<>excluded.is_dir
                OR files.scope_id<>excluded.scope_id
                OR (excluded.native_file_id IS NOT NULL
                    AND files.native_file_id IS NOT excluded.native_file_id)",
        )
        .and_then(|mut s| s.execute(params![
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
            ]))
        .map_err(|e| e.to_string())?;
        let fid = match &existing {
            None => c.last_insert_rowid(),
            Some((fid, old_name)) if old_name != &item.name => *fid,
            Some(_) => continue,
        };
        c.prepare_cached(
            "INSERT INTO index_outbox(file_id, operation, created_ms) VALUES (?1,'upsert',?2)",
        )
        .and_then(|mut s| s.execute(params![fid, util::now_ms()]))
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

/// 校准用的库内快照：路径哈希 → (file_id, size, modified_ms)。
/// 用 64 位哈希代替完整路径字符串，百万级文件内存从数百 MB 降到几十 MB。
pub type KnownMap = HashMap<u64, (i64, i64, i64)>;

fn path_key(path: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut h);
    h.finish()
}

fn mtime_ms(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// 遍历过程的可变上下文（替代一长串 &mut 参数）。
struct Walk<'a, F: FnMut(Vec<FileUpsert>)> {
    opts: &'a ScopeOptions,
    count_only: bool,
    batch: Vec<FileUpsert>,
    on_batch: F,
    errors: u64,
    seen: u64,
    total: u64,
    ctx: Option<&'a JobContext>,
    /// 校准模式：与库内快照比对，未变化的条目直接跳过（不取文件标识、不写库）；
    /// 遍历到的条目从快照中移除，结束后剩余的即为磁盘上已不存在的条目。
    known: Option<&'a mut KnownMap>,
}

impl<'a, F: FnMut(Vec<FileUpsert>)> Walk<'a, F> {
    /// 条目是否需要（重新）写库；校准模式下未变化返回 false。
    fn changed(&mut self, path: &Path, size: i64, modified: i64) -> bool {
        match self.known.as_deref_mut() {
            None => true,
            Some(known) => match known.remove(&path_key(&util::normalize_path(path))) {
                Some((_, s, m)) => s != size || m != modified,
                None => true,
            },
        }
    }

    fn push(&mut self, u: FileUpsert) {
        self.batch.push(u);
        if self.batch.len() >= 200 {
            (self.on_batch)(std::mem::take(&mut self.batch));
        }
    }

    /// 遍历目录树：受保护目录、`$` 开头系统目录、应用数据目录、隐藏目录、排除规则整树跳过；
    /// 不跟随符号链接 / 目录联接（避免循环与重复计数）。
    fn walk(&mut self, dir: &Path) {
        let read = match std::fs::read_dir(dir) {
            Ok(r) => r,
            Err(_) => {
                self.errors += 1;
                return;
            }
        };
        for entry in read.flatten() {
            if let Some(ctx) = self.ctx {
                ctx.wait_if_paused();
                if ctx.cancelled() {
                    return;
                }
                self.seen += 1;
                if self.count_only && self.seen % 20000 == 0 {
                    // 统计阶段持续给出反馈，避免看起来像卡死
                    ctx.progress(0.0, &format!("正在遍历，已发现 {} 项", self.seen));
                }
            }
            let path = entry.path();
            // Windows 上目录枚举自带元数据，无需额外系统调用
            let Ok(meta) = entry.metadata() else {
                self.errors += 1;
                continue;
            };
            let attrs = {
                use std::os::windows::fs::MetadataExt;
                meta.file_attributes()
            };
            let name = entry.file_name().to_string_lossy().to_string();
            let excluded = (!self.opts.include_hidden && util::is_hidden(attrs))
                || crate::scope::segment_excluded(&name, self.opts);
            if meta.is_dir() {
                if excluded
                    || name.starts_with('$')
                    || crate::scope::is_protected(&name)
                    || util::is_app_data_path(&util::normalize_path(&path))
                {
                    continue; // 整树跳过
                }
                if self.count_only {
                    self.total += 1;
                } else if self.changed(&path, 0, mtime_ms(&meta)) {
                    if let Some(mut u) = stat_with_meta(&path, &meta, 0, self.opts) {
                        u.kind = "folder".into();
                        u.is_dir = true;
                        u.ext = String::new();
                        u.size = 0;
                        self.push(u);
                    }
                }
                self.walk(&path);
            } else if !excluded {
                if self.count_only {
                    self.total += 1;
                } else if meta.file_type().is_symlink() {
                    // 链接：取目标元数据记录为单个条目，不深入
                    if let Some(u) = stat_entry(&path, 0, self.opts) {
                        if self.changed(&path, u.size, u.modified_ms) {
                            self.push(u);
                        }
                    }
                } else if self.changed(&path, meta.len() as i64, mtime_ms(&meta)) {
                    if let Some(u) = stat_with_meta(&path, &meta, 0, self.opts) {
                        self.push(u);
                    }
                }
            }
        }
    }

    fn finish(mut self) -> (u64, u64) {
        if !self.batch.is_empty() {
            (self.on_batch)(std::mem::take(&mut self.batch));
        }
        (self.total, self.errors)
    }
}

/// 阶段一：快速统计范围总数（仅枚举目录项，不打开文件取标识）。
fn count_dir(root: &Path, opts: &ScopeOptions, ctx: Option<&JobContext>) -> (u64, u64) {
    let mut w = Walk {
        opts,
        count_only: true,
        batch: Vec::new(),
        on_batch: |_: Vec<FileUpsert>| {},
        errors: 0,
        seen: 0,
        total: 0,
        ctx,
        known: None,
    };
    w.walk(root);
    w.finish()
}

/// 阶段二：流式收集元数据，每凑满一批即回调写出；返回遍历错误数。
/// 传入 `known` 时为校准模式（只写出新增或变化的条目）。
fn walk_dir(
    root: &Path,
    opts: &ScopeOptions,
    ctx: Option<&JobContext>,
    known: Option<&mut KnownMap>,
    on_batch: impl FnMut(Vec<FileUpsert>),
) -> u64 {
    let mut w = Walk {
        opts,
        count_only: false,
        batch: Vec::with_capacity(200),
        on_batch,
        errors: 0,
        seen: 0,
        total: 0,
        ctx,
        known,
    };
    w.walk(root);
    w.finish().1
}

/// 按 file_id 分块删除文件行，并登记索引删除事件（先登记再删行，否则事件查不到行）。
pub fn purge_file_ids(db: &DbHandle, ids: &[i64]) {
    for chunk in ids.chunks(500) {
        let list = chunk
            .iter()
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(",");
        db.with_conn(|c| {
            c.execute("BEGIN IMMEDIATE", []).ok();
            c.execute(
                &format!(
                    "INSERT INTO index_outbox(file_id, operation, created_ms)
                     SELECT file_id, 'delete', ?1 FROM files WHERE file_id IN ({list})"
                ),
                [util::now_ms()],
            )
            .ok();
            c.execute(&format!("DELETE FROM files WHERE file_id IN ({list})"), [])
                .ok();
            c.execute("COMMIT", []).ok();
        });
        // 块间让出，避免长时间独占后台连接
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

/// 清空某范围的全部文件行（分块进行，不阻塞界面连接，并同步删除索引文档）。
pub fn purge_scope(db: &DbHandle, scope_id: i64) {
    loop {
        let ids: Vec<i64> = db.with_conn(|c| {
            c.prepare("SELECT file_id FROM files WHERE scope_id=?1 LIMIT 5000")
                .and_then(|mut s| {
                    s.query_map([scope_id], |r| r.get::<_, i64>(0))
                        .map(|rows| rows.filter_map(|x| x.ok()).collect())
                })
                .unwrap_or_default()
        });
        if ids.is_empty() {
            break;
        }
        purge_file_ids(db, &ids);
    }
}

pub fn start_scan_job(state: &crate::AppState, scope_id: i64) -> Result<(), String> {
    start_scan_job_inner(state, scope_id, false)
}

/// `purge_first`：重建范围时先在任务线程里分块清空旧行，再重新扫描。
fn start_scan_job_inner(
    state: &crate::AppState,
    scope_id: i64,
    purge_first: bool,
) -> Result<(), String> {
    let db = state.bg_db.clone();
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
        move |ctx| {
            if purge_first {
                ctx.progress(0.0, "正在清空旧索引…");
                purge_scope(&db_for_body, scope_id);
            }
            run_initial_scan(ctx, db_for_body, scope_id, root, opts)
        },
    )?;
    state.db.with_conn(|c| {
        c.execute("UPDATE scopes SET state='scanning' WHERE scope_id=?1", [scope_id])
            .ok();
    });
    Ok(())
}

fn run_initial_scan(
    ctx: JobContext,
    db: Arc<DbHandle>,
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
    let walk_errors = walk_dir(Path::new(&root), &opts, Some(&ctx), None, |mut batch| {
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
        // 批间让出：给其他后台写入获取数据库锁的机会
        std::thread::sleep(std::time::Duration::from_millis(8));
    });
    failed += walk_errors;

    if ctx.cancelled() {
        ctx.finish_cancelled();
        db.with_conn(|c| {
            c.execute("UPDATE scopes SET state='idle' WHERE scope_id=?1", [scope_id])
                .ok();
        });
        return;
    }
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
/// `scope_id` 为 None 时依次校准全部启用范围（同一任务内串行，避免同类任务去重导致漏校）。
pub fn start_recalibrate_job(state: &crate::AppState, scope_id: Option<i64>) -> Result<(), String> {
    let db = state.bg_db.clone();
    let targets: Vec<(i64, String, String)> = db.with_conn(|c| {
        let mut stmt = c
            .prepare(
                "SELECT scope_id, path, options_json FROM scopes
                 WHERE enabled=1 AND (?1 IS NULL OR scope_id=?1) ORDER BY scope_id",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([scope_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .map_err(|e| e.to_string())?;
        Ok::<_, String>(rows.filter_map(|x| x.ok()).collect())
    })?;
    if targets.is_empty() {
        return Ok(());
    }
    let db_for_body = db.clone();
    state.engine.start(
        "recalibrate",
        serde_json::json!({ "scopeId": scope_id }),
        move |ctx| {
            let mut removed = 0usize;
            let mut errors = 0u64;
            for (sid, root, opts_json) in targets {
                let opts = scope_options(&opts_json);
                match recalibrate_scope(&ctx, &db_for_body, sid, &root, &opts) {
                    Ok((r, e)) => {
                        removed += r;
                        errors += e;
                    }
                    Err(msg) => tracing::warn!("范围 {sid} 校准跳过：{msg}"),
                }
                if ctx.cancelled() {
                    ctx.finish_cancelled();
                    return;
                }
            }
            ctx.finish_ok(Some(
                serde_json::json!({ "scopeId": scope_id, "removed": removed, "errors": errors })
                    .to_string(),
            ));
        },
    )?;
    Ok(())
}

/// 校准单个范围：只写入新增 / 变化的条目，删除磁盘上已不存在的条目。
/// 返回 (删除数, 遍历错误数)；范围根不可访问或任务取消时不做任何删除。
fn recalibrate_scope(
    ctx: &JobContext,
    db: &Arc<DbHandle>,
    scope_id: i64,
    root: &str,
    opts: &ScopeOptions,
) -> Result<(usize, u64), String> {
    // 根目录不可访问（移动硬盘拔出、网络断开）时不得把整范围判定为已删除
    if std::fs::read_dir(root).is_err() {
        return Err("范围根目录不可访问".into());
    }
    let mut known: KnownMap = db.with_conn(|c| {
        let mut stmt = c
            .prepare("SELECT file_id, path, size, modified_ms FROM files WHERE scope_id=?1")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([scope_id], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, i64>(3)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        let mut m = KnownMap::new();
        for (fid, path, size, mtime) in rows.flatten() {
            m.insert(path_key(&path), (fid, size, mtime));
        }
        Ok::<_, String>(m)
    })?;
    let baseline = known.len();
    let mut changed = 0u64;
    let errors = walk_dir(Path::new(root), opts, Some(ctx), Some(&mut known), |batch| {
        if ctx.cancelled() {
            return;
        }
        let mut fixed: Vec<FileUpsert> = Vec::with_capacity(batch.len());
        for mut u in batch {
            u.scope_id = scope_id;
            fixed.push(u);
        }
        changed += fixed.len() as u64;
        db.with_conn(|c| {
            c.execute("BEGIN IMMEDIATE", []).ok();
            let _ = upsert_batch(c, &fixed);
            c.execute("COMMIT", []).ok();
        });
        ctx.progress(0.0, &format!("校准对比中，已更新 {changed} 项"));
        std::thread::sleep(std::time::Duration::from_millis(5));
    });
    if ctx.cancelled() {
        return Ok((0, errors));
    }

    // 快照中剩余的条目在本次遍历中未出现：磁盘上已删除或已被排除规则覆盖
    let removed: Vec<i64> = known.values().map(|v| v.0).collect();
    drop(known);
    purge_file_ids(db, &removed);
    db.with_conn(|c| {
        c.execute(
            "UPDATE scopes SET baseline_at=?2, last_scan_ms=?3, state='idle' WHERE scope_id=?1",
            params![scope_id, util::now_rfc3339(), util::now_ms()],
        )
        .ok();
    });
    tracing::info!(
        "范围 {scope_id} 校准完成：库内 {baseline} 项，更新 {changed} 项，删除 {} 项",
        removed.len()
    );
    Ok((removed.len(), errors))
}

/// 重建范围：在后台任务中分块清空该范围的旧行后重扫（FR18）。
pub fn start_rebuild_job(state: &crate::AppState, scope_id: i64) -> Result<(), String> {
    start_scan_job_inner(state, scope_id, true)
}

#[tauri::command]
pub async fn cmd_scan(
    state: tauri::State<'_, Arc<crate::AppState>>,
    scope_id: i64,
) -> Result<(), String> {
    start_scan_job(&state, scope_id)
}

#[tauri::command]
pub async fn cmd_recalibrate(
    state: tauri::State<'_, Arc<crate::AppState>>,
    scope_id: i64,
) -> Result<(), String> {
    start_recalibrate_job(&state, Some(scope_id))
}

#[tauri::command]
pub async fn cmd_rebuild(
    state: tauri::State<'_, Arc<crate::AppState>>,
    scope_id: i64,
) -> Result<(), String> {
    start_rebuild_job(&state, scope_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn_with_scope() -> Connection {
        let c = crate::db::open_in_memory().unwrap();
        c.execute(
            r"INSERT INTO scopes(scope_id, path, added_at) VALUES (1, 'D:\s', 'now')",
            [],
        )
        .unwrap();
        c
    }

    fn row(name: &str, size: i64, modified_ms: i64) -> FileUpsert {
        FileUpsert {
            scope_id: 1,
            name: name.into(),
            path: format!(r"D:\s\{name}"),
            ext: "txt".into(),
            kind: "doc".into(),
            size,
            created_ms: 1,
            modified_ms,
            attributes: 0,
            is_dir: false,
            native_serial: None,
            native_file_id: None,
            parent_path: r"D:\s".into(),
        }
    }

    fn outbox_count(c: &Connection, op: &str) -> i64 {
        c.query_row(
            "SELECT COUNT(*) FROM index_outbox WHERE operation=?1",
            [op],
            |r| r.get(0),
        )
        .unwrap()
    }

    #[test]
    fn upsert_enqueues_only_new_rows() {
        let c = conn_with_scope();
        upsert_batch(&c, &[row("a.txt", 10, 100)]).unwrap();
        assert_eq!(outbox_count(&c, "upsert"), 1);
        let updated: i64 = c
            .query_row("SELECT updated_ms FROM files", [], |r| r.get(0))
            .unwrap();

        // 完全未变化：不入队、不写库
        std::thread::sleep(std::time::Duration::from_millis(5));
        upsert_batch(&c, &[row("a.txt", 10, 100)]).unwrap();
        assert_eq!(outbox_count(&c, "upsert"), 1);
        let updated_again: i64 = c
            .query_row("SELECT updated_ms FROM files", [], |r| r.get(0))
            .unwrap();
        assert_eq!(updated, updated_again);

        // 内容变化：更新元数据并标记全文待重抽，但文件名未变无需重建索引文档
        upsert_batch(&c, &[row("a.txt", 20, 200)]).unwrap();
        assert_eq!(outbox_count(&c, "upsert"), 1);
        let (size, ft): (i64, String) = c
            .query_row("SELECT size, fulltext_state FROM files", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(size, 20);
        assert_eq!(ft, "pending");
    }

    #[test]
    fn purge_enqueues_delete_before_removing_rows() {
        let c = conn_with_scope();
        upsert_batch(&c, &[row("a.txt", 1, 1), row("b.txt", 1, 1)]).unwrap();
        let db = DbHandle::new(c);
        purge_scope(&db, 1);
        db.with_conn(|c| {
            let left: i64 = c
                .query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0))
                .unwrap();
            assert_eq!(left, 0);
            assert_eq!(outbox_count(c, "delete"), 2);
        });
    }

    #[test]
    fn recalibrate_walk_emits_only_changed_and_reports_missing() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");
        std::fs::write(&a, b"hello").unwrap();
        std::fs::write(&b, b"world!").unwrap();
        let meta_a = std::fs::metadata(&a).unwrap();

        let mut known = KnownMap::new();
        // a.txt 与库内一致；b.txt 库内没有；gone.txt 只在库内
        known.insert(
            path_key(&util::normalize_path(&a)),
            (1, meta_a.len() as i64, mtime_ms(&meta_a)),
        );
        known.insert(
            path_key(&util::normalize_path(&dir.path().join("gone.txt"))),
            (3, 1, 1),
        );

        let mut emitted: Vec<String> = Vec::new();
        let opts = ScopeOptions::default();
        walk_dir(dir.path(), &opts, None, Some(&mut known), |batch| {
            emitted.extend(batch.into_iter().map(|u| u.name));
        });
        assert_eq!(emitted, vec!["b.txt".to_string()]);
        let missing: Vec<i64> = known.values().map(|v| v.0).collect();
        assert_eq!(missing, vec![3]);
    }
}
