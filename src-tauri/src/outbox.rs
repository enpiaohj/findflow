use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use rusqlite::params;

use crate::settings::DbHandle;

/// 每批读取的事件数。
const BATCH: usize = 2000;
/// 未提交文档达到该数量立即提交（批量重放期间减少提交次数）。
const COMMIT_DOCS: usize = 20_000;
/// 有未提交改动时的最长提交间隔。
const COMMIT_MAX_INTERVAL: Duration = Duration::from_secs(5);
/// 队列已空时的最短提交间隔（零散变更尽快可搜，又不致每秒多次提交）。
const COMMIT_IDLE_INTERVAL: Duration = Duration::from_secs(1);

/// 单次消费循环：持续处理事件，直到停止或异常。由外层 catch_unwind 守护。
fn consume_tick(state: &Arc<crate::AppState>, bg: &DbHandleArc, stop: &AtomicBool) {
    let mut last_commit = Instant::now();
    let mut last_checkpoint = Instant::now();
    let mut dirty: usize = 0;
    // 存在已写入 writer、但尚未成功提交的事件。提交失败时这些文档只在内存里的
    // writer 队列中，事件必须保留，否则进程一退就永久缺失。
    let mut uncommitted = false;
    loop {
        if stop.load(Ordering::SeqCst) {
            if dirty > 0 || uncommitted {
                commit_and_reload(state, bg);
            }
            return;
        }
        let batch = bg.with_conn(|c| -> Result<Vec<(i64, i64)>, String> {
            let mut stmt = c
                .prepare_cached(
                    "SELECT event_id, file_id FROM index_outbox
                     WHERE state='pending' ORDER BY event_id LIMIT ?1",
                )
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map([BATCH as i64], |r| Ok((r.get(0)?, r.get(1)?)))
                .map_err(|e| e.to_string())?;
            Ok(rows.filter_map(|x| x.ok()).collect())
        });
        match batch {
            Ok(events) if events.is_empty() => {
                if (dirty > 0 || uncommitted) && last_commit.elapsed() >= COMMIT_IDLE_INTERVAL {
                    if commit_and_reload(state, bg) {
                        dirty = 0;
                        uncommitted = false;
                    }
                    last_commit = Instant::now();
                }
                // 空闲期定期截断 WAL，防止无限膨胀
                if last_checkpoint.elapsed() >= Duration::from_secs(300) {
                    bg.with_conn(|c| {
                        c.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);").ok();
                    });
                    last_checkpoint = Instant::now();
                }
                std::thread::sleep(Duration::from_millis(250));
            }
            Ok(events) => {
                let min_id = events.first().map(|e| e.0).unwrap_or(0);
                let max_id = events.last().map(|e| e.0).unwrap_or(0);
                // 同一文件的多次事件只需处理一次：最终以 files 表当前行为准
                let mut fids: Vec<i64> = events.iter().map(|e| e.1).collect();
                fids.sort_unstable();
                fids.dedup();
                dirty += apply_batch(bg, state, &fids);
                // 只标记「已写入 writer」，不删除——提交成功后由 commit_and_reload 删除。
                // 此前在这里直接删除事件，一旦提交失败（杀毒软件瞬时占用、磁盘满），
                // 文档还没落盘而事件已消失，索引与数据库就静默不一致了。
                // 标记为 applied 后不会被本循环重读（只取 pending），
                // 进程重启时再由 recover_applied 重置回 pending 重放。
                mark_applied(bg, min_id, max_id);
                uncommitted = true;
                if dirty >= COMMIT_DOCS || last_commit.elapsed() >= COMMIT_MAX_INTERVAL {
                    if commit_and_reload(state, bg) {
                        uncommitted = false;
                    }
                    dirty = 0;
                    last_commit = Instant::now();
                }
                // 批间让出，避免饿死其他后台写入
                std::thread::sleep(Duration::from_millis(3));
            }
            Err(_) => std::thread::sleep(Duration::from_millis(1000)),
        }
    }
}

/// 把本批事件标记为「已写入 writer」（尚未提交）。事件此时不能删除：
/// 文档还在 writer 的内存队列里，提交失败就等于丢失。
fn mark_applied(bg: &DbHandleArc, min_id: i64, max_id: i64) {
    bg.with_conn(|c| {
        c.execute(
            "UPDATE index_outbox SET state='applied' WHERE event_id BETWEEN ?1 AND ?2",
            params![min_id, max_id],
        )
        .ok();
    });
}

/// 崩溃恢复：上次运行中已写入 writer 但未提交的事件，其文档并未真正落盘
/// （writer 队列只在内存中）。重置为 pending 以便重新写入；
/// apply_batch 是先删后加，对同一文件重复应用是幂等的。
fn recover_applied(bg: &DbHandleArc) {
    bg.with_conn(|c| {
        c.execute(
            "UPDATE index_outbox SET state='pending' WHERE state='applied'",
            [],
        )
        .ok();
    });
}

/// 提交并清理。只有提交成功才删除已写入的事件：提交失败时文档尚未落盘，
/// 删掉事件等于永久丢失，只能等下一次全量重建才能找回。返回是否提交成功。
fn commit_and_reload(state: &crate::AppState, bg: &DbHandleArc) -> bool {
    // 写入阶段写入器就已失效并被重建：此前写入的文档丢在旧写入器里，
    // 绝不能在新写入器上「提交成功」后把对应事件删掉——先重置回 pending 重放。
    if state.index.take_writer_reset() {
        tracing::warn!("索引写入器曾失效，已写入未提交的事件重置为待处理并重放");
        recover_applied(bg);
        return false;
    }
    match state.index.commit() {
        Ok(()) => {
            // 一次成功提交会落盘 writer 队列中的全部文档，故可清掉所有已写入事件
            bg.with_conn(|c| {
                c.execute("DELETE FROM index_outbox WHERE state='applied'", [])
                    .ok();
            });
            state.index.reload();
            true
        }
        Err(e) => {
            tracing::warn!("索引提交失败，事件保留待重放: {e}");
            // 提交失败会丢弃写入器：旧写入器里未落盘的文档已丢失，立即重放而不是等重启
            if state.index.take_writer_reset() {
                recover_applied(bg);
            }
            false
        }
    }
}

type DbHandleArc = Arc<DbHandle>;

/// outbox 消费线程：SQLite 事务先落元数据与待索引事件，后台再提交 Tantivy，
/// 异常退出后可继续消费，避免两套存储静默不一致（设计文档 §7.3）。
/// 消费线程带 panic 隔离：单批异常只损失该批，线程自动重启继续消费。
pub fn spawn_consumer(state: Arc<crate::AppState>, bg: DbHandleArc, stop: Arc<AtomicBool>) {
    std::thread::Builder::new()
        .name("outbox-consumer".into())
        .spawn(move || {
            // 消费线程降优先级，避免与界面争抢 CPU
            #[cfg(windows)]
            unsafe {
                use windows::Win32::System::Threading::{
                    SetThreadPriority, THREAD_PRIORITY_BELOW_NORMAL,
                };
                let _ = SetThreadPriority(
                    windows::Win32::Foundation::HANDLE(std::ptr::null_mut()),
                    THREAD_PRIORITY_BELOW_NORMAL,
                );
            }
            let last_commit = std::time::Instant::now();
            let _ = last_commit;
            // 崩溃恢复：上次运行中已写入 writer 但未提交的事件，其文档并未真正落盘
            // （writer 队列只在内存中）。重置为 pending 重新写入；apply_batch 是
            // 先删后加，对同一文件重复应用是幂等的。
            recover_applied(&bg);
            loop {
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    consume_tick(&state, &bg, &stop);
                }));
                match result {
                    Ok(_) if stop.load(Ordering::SeqCst) => break,
                    Ok(_) => continue,
                    Err(_) => {
                        tracing::error!("outbox 消费线程异常退出，3 秒后自动重启");
                        std::thread::sleep(Duration::from_secs(3));
                    }
                }
            }
        })
        .ok();
}

/// 按文件批量同步到索引：行存在则重写文档（先删后加，避免重复文档），行已删除则删文档。
/// 返回写入的改动数。
fn apply_batch(db: &DbHandle, state: &crate::AppState, fids: &[i64]) -> usize {
    let mut rows: std::collections::HashMap<i64, (String, Option<String>, Option<String>)> =
        std::collections::HashMap::with_capacity(fids.len());
    for chunk in fids.chunks(500) {
        let list = chunk
            .iter()
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(",");
        db.with_conn(|c| {
            let sql = format!(
                "SELECT f.file_id, f.name,
                        CASE WHEN f.fulltext_state='done' THEN ct.content_text END,
                        CASE WHEN f.ocr_state='done' THEN ot.ocr_text END
                 FROM files f
                 LEFT JOIN content_text ct ON ct.file_id = f.file_id
                 LEFT JOIN ocr_text ot ON ot.file_id = f.file_id
                 WHERE f.file_id IN ({list})"
            );
            if let Ok(mut stmt) = c.prepare(&sql) {
                if let Ok(it) = stmt.query_map([], |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, Option<String>>(2)?,
                        r.get::<_, Option<String>>(3)?,
                    ))
                }) {
                    for (fid, name, content, ocr) in it.flatten() {
                        rows.insert(fid, (name, content, ocr));
                    }
                }
            }
        });
    }
    for fid in fids {
        state.index.delete(*fid as u64);
        if let Some((name, content, ocr)) = rows.get(fid) {
            let doc = state
                .index
                .build_doc(*fid as u64, name, content.as_deref(), ocr.as_deref());
            let _ = state.index.add(doc);
        }
    }
    fids.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::SettingsStore;
    use std::sync::OnceLock;

    /// 用临时文件库构造 AppState：内存库的多连接互不可见，outbox 需要同一份数据。
    fn test_state(dir: &std::path::Path) -> crate::AppState {
        let db_path = dir.join("t.db");
        let store = Arc::new(SettingsStore::new(crate::db::open(&db_path).unwrap()));
        let bg = Arc::new(DbHandle::new(crate::db::open(&db_path).unwrap()));
        crate::AppState {
            db: store.clone(),
            bg_db: bg.clone(),
            stats_db: bg.clone(),
            data_dir: dir.to_path_buf(),
            engine: crate::jobs::JobEngine::new(store, bg),
            index: crate::searchindex::SearchIndex::open_or_create(&dir.join("idx")).unwrap(),
            watch: OnceLock::new(),
        }
    }

    fn seed_events(bg: &DbHandle, n: i64) {
        bg.with_conn(|c| {
            c.execute(
                "INSERT INTO scopes(scope_id, path, added_at) VALUES (1, 'D:/a', 'now')",
                [],
            )
            .unwrap();
            for i in 0..n {
                let name = format!("f{i}.txt");
                c.execute(
                    "INSERT INTO files(scope_id, name, path, kind) VALUES (1, ?1, ?2, 'doc')",
                    params![name, format!("D:/a/{name}")],
                )
                .unwrap();
                let fid = c.last_insert_rowid();
                c.execute(
                    "INSERT INTO index_outbox(file_id, operation, created_ms)
                     VALUES (?1, 'upsert', 0)",
                    [fid],
                )
                .unwrap();
            }
        });
    }

    fn count(bg: &DbHandle, state: Option<&str>) -> i64 {
        bg.with_conn(|c| {
            match state {
                Some(s) => c
                    .query_row(
                        "SELECT COUNT(*) FROM index_outbox WHERE state=?1",
                        [s],
                        |r| r.get(0),
                    )
                    .unwrap(),
                None => c
                    .query_row("SELECT COUNT(*) FROM index_outbox", [], |r| r.get(0))
                    .unwrap(),
            }
        })
    }

    /// 核心不变式：事件在提交成功之前绝不能消失。
    /// 此前实现是先删事件再提交，提交失败（杀毒软件瞬时占用、磁盘满）时
    /// 文档还没落盘而事件已删除，索引与数据库静默不一致。
    #[test]
    fn writer_reset_replays_applied_events_instead_of_deleting_them() {
        crate::testutil::retry_on_index_io(|| {
            let dir = tempfile::tempdir().unwrap();
            let state = test_state(dir.path());
            seed_events(&state.bg_db, 3);
            mark_applied(&state.bg_db, 1, 3);
            // 模拟写入器失效被丢弃：旧写入器里这 3 篇文档已丢失
            {
                let mut slot = state.index.writer.lock().unwrap();
                state.index.discard_writer(&mut slot);
            }
            // 新写入器上的提交会「成功」，但绝不能据此删除事件：先重置回 pending 重放
            assert!(!commit_and_reload(&state, &state.bg_db));
            assert_eq!(count(&state.bg_db, None), 3, "事件不得被删除");
            assert_eq!(count(&state.bg_db, Some("pending")), 3);
            // 重放（重新写入并标记）后正常提交、清理
            mark_applied(&state.bg_db, 1, 3);
            assert!(commit_and_reload(&state, &state.bg_db));
            assert_eq!(count(&state.bg_db, None), 0);
        });
    }

    #[test]
    fn events_survive_until_commit_succeeds() {
        crate::testutil::retry_on_index_io(|| {
            let dir = tempfile::tempdir().unwrap();
            let state = test_state(dir.path());
            seed_events(&state.bg_db, 3);
            assert_eq!(count(&state.bg_db, Some("pending")), 3);

            // 写入 writer 后只标记状态，不删除
            mark_applied(&state.bg_db, 1, 3);
            assert_eq!(count(&state.bg_db, None), 3, "事件不得被删除");
            assert_eq!(count(&state.bg_db, Some("applied")), 3);
            assert_eq!(count(&state.bg_db, Some("pending")), 0);

            // 提交成功后才清理
            assert!(commit_and_reload(&state, &state.bg_db));
            assert_eq!(count(&state.bg_db, None), 0, "提交成功后事件被清理");
        });
    }

    /// 崩溃恢复：进程在「已写入但未提交」时退出，下次启动必须重放这些事件，
    /// 否则那批文档永远不在索引里。
    #[test]
    fn recovery_replays_events_that_never_committed() {
        crate::testutil::retry_on_index_io(|| {
            let dir = tempfile::tempdir().unwrap();
            let state = test_state(dir.path());
            seed_events(&state.bg_db, 3);

            mark_applied(&state.bg_db, 1, 3);
            assert_eq!(count(&state.bg_db, Some("applied")), 3);
            // 模拟进程退出（未提交），下次启动执行恢复
            recover_applied(&state.bg_db);
            assert_eq!(count(&state.bg_db, Some("pending")), 3);
            assert_eq!(count(&state.bg_db, None), 3, "恢复后事件仍完整，可重新写入");
        });
    }
}
