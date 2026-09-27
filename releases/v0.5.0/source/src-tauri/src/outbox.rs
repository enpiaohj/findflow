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
    loop {
        if stop.load(Ordering::SeqCst) {
            if dirty > 0 {
                let _ = state.index.commit();
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
                if dirty > 0 && last_commit.elapsed() >= COMMIT_IDLE_INTERVAL {
                    commit_and_reload(state);
                    dirty = 0;
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
                // 本批是按 event_id 升序取的最小 N 条，区间内不会有其他待处理事件；
                // 新事件的 rowid 必然大于当前最大值，区间删除安全且只需一次索引范围扫描
                bg.with_conn(|c| {
                    c.execute(
                        "DELETE FROM index_outbox WHERE event_id BETWEEN ?1 AND ?2",
                        params![min_id, max_id],
                    )
                    .ok();
                });
                if dirty >= COMMIT_DOCS || last_commit.elapsed() >= COMMIT_MAX_INTERVAL {
                    commit_and_reload(state);
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

fn commit_and_reload(state: &crate::AppState) {
    if let Err(e) = state.index.commit() {
        tracing::warn!("索引提交失败: {e}");
    }
    state.index.reload();
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
