use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use rusqlite::params;

use crate::settings::DbHandle;

/// 单次消费循环：处理一批事件后返回。由外层 catch_unwind 守护。
fn consume_tick(state: &Arc<crate::AppState>, bg: &DbHandleArc, stop: &AtomicBool) {
    let mut last_commit = Instant::now() - Duration::from_secs(10);
    loop {
        if stop.load(Ordering::SeqCst) {
            return;
        }
        let batch = bg.with_conn(|c| -> Result<Vec<(i64, i64, String)>, String> {
            let mut stmt = c
                .prepare(
                    "SELECT event_id, file_id, operation FROM index_outbox
                     WHERE state='pending' ORDER BY event_id LIMIT 400",
                )
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                .map_err(|e| e.to_string())?;
            let all: Vec<(i64, i64, String)> = rows.filter_map(|x| x.ok()).collect();
            Ok(all)
        });
        match batch {
            Ok(events) if events.is_empty() => {
                // 空闲期定期截断 WAL，防止无限膨胀
                if last_commit.elapsed() >= Duration::from_secs(300) {
                    bg.with_conn(|c| {
                        c.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);").ok();
                    });
                    last_commit = Instant::now();
                }
                std::thread::sleep(Duration::from_millis(250));
            }
            Ok(events) => {
                let mut done_ids: Vec<i64> = Vec::with_capacity(events.len());
                for (event_id, fid, op) in &events {
                    apply_event(bg, state, *fid, op);
                    done_ids.push(*event_id);
                }
                // 单条语句批量删除已应用事件，减少持锁次数
                if !done_ids.is_empty() {
                    let list = done_ids
                        .iter()
                        .map(|i| i.to_string())
                        .collect::<Vec<_>>()
                        .join(",");
                    bg.with_conn(|c| {
                        c.execute(
                            &format!("DELETE FROM index_outbox WHERE event_id IN ({})", list),
                            [],
                        )
                        .ok();
                    });
                }
                // 提交批量化：满批立即提交；不满批时隔 2 秒再提交
                if events.len() >= 400 || last_commit.elapsed() >= Duration::from_secs(2) {
                    let _ = state.index.commit();
                    state.index.reload();
                    last_commit = Instant::now();
                }
                // 批间让出，避免饿死界面命令
                std::thread::sleep(Duration::from_millis(3));
            }
            Err(_) => std::thread::sleep(Duration::from_millis(1000)),
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

fn apply_event(db: &crate::settings::DbHandle, state: &crate::AppState, fid: i64, op: &str) {
    if op == "delete" {
        state.index.delete(fid as u64);
        return;
    }
    // upsert：重读行，内容/OCR 已就绪则一并写入。
    let row: Option<(String, String, String, String)> = db.with_conn(|c| {
        c.query_row(
            "SELECT name, COALESCE(f.fulltext_state,'none'), COALESCE(cs.content_hash,''),
                    COALESCE(f.ocr_state,'none')
             FROM files f
             LEFT JOIN content_state cs ON cs.file_id = f.file_id
             WHERE f.file_id=?1",
            params![fid],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ))
            },
        )
        .ok()
    });
    let Some((name, ft_state, _hash, ocr_state)) = row else {
        state.index.delete(fid as u64);
        return;
    };
    let _content = if ft_state == "done" {
        db.with_conn(|c| {
            c.query_row(
                "SELECT content_text FROM content_text WHERE file_id=?1",
                params![fid],
                |r| r.get::<_, String>(0),
            )
            .ok()
        })
    } else {
        None
    };
    let content = _content.as_deref();
    let ocr = if ocr_state == "done" {
        db.with_conn(|c| {
            c.query_row(
                "SELECT ocr_text FROM ocr_text WHERE file_id=?1",
                params![fid],
                |r| r.get::<_, String>(0),
            )
            .ok()
        })
    } else {
        None
    };
    let doc = state.index.build_doc(fid as u64, &name, content, ocr.as_deref());
    let _ = state.index.add(doc);
}
