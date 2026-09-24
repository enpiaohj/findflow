use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use notify::{RecursiveMode, Watcher};
use std::path::Path;
use rusqlite::params;

use crate::scope::scope_options;

pub enum WatchMsg {
    Event(notify::Event),
}

/// 增量监听池：每范围一个 notify watcher，批量合并事件后落到索引（FR04）。
pub struct WatchPool {
    tx: Sender<WatchMsg>,
    pub watchers: Arc<Mutex<HashMap<i64, notify::RecommendedWatcher>>>,
}

impl WatchPool {
    pub fn new(state: Arc<crate::AppState>) -> Self {
        let (tx, rx) = channel::<WatchMsg>();
        // 批处理线程：250ms 窗口合并，rename 配对，落库 + outbox。
        std::thread::Builder::new()
            .name("watch-batcher".into())
            .spawn(move || batch_loop(state, rx))
            .ok();
        Self {
            tx,
            watchers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn watch(&self, scope_id: i64, path: &str) -> Result<(), String> {
        let mut map = self.watchers.lock().unwrap();
        if map.contains_key(&scope_id) {
            return Ok(());
        }
        let tx = self.tx.clone();
        let mut watcher = notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
            if let Ok(ev) = res {
                let _ = tx.send(WatchMsg::Event(ev));
            }
        })
        .map_err(|e| e.to_string())?;
        watcher
            .watch(Path::new(path), RecursiveMode::Recursive)
            .map_err(|e| format!("无法监听该目录（可能是网络路径）：{e}"))?;
        map.insert(scope_id, watcher);
        Ok(())
    }

    pub fn unwatch(&self, scope_id: i64) {
        self.watchers.lock().unwrap().remove(&scope_id);
    }
}

fn batch_loop(state: Arc<crate::AppState>, rx: Receiver<WatchMsg>) {
    let mut pending: Vec<String> = Vec::new();
    let mut rename_from: Option<String> = None;
    loop {
        match rx.recv_timeout(Duration::from_millis(250)) {
            Ok(WatchMsg::Event(ev)) => {
                if ev.need_rescan() {
                    // 缓冲区溢出：调度全量校准
                    schedule_calibration(&state);
                    continue;
                }
                for path in ev.paths.clone() {
                    let p = crate::util::normalize_path(&path);
                    match ev.kind {
                        notify::event::EventKind::Modify(notify::event::ModifyKind::Name(
                            notify::event::RenameMode::From,
                        )) => {
                            rename_from = Some(p);
                        }
                        notify::event::EventKind::Modify(notify::event::ModifyKind::Name(
                            notify::event::RenameMode::Both,
                        )) => {
                            // notify 7 的 Both 事件带 from/to 两个路径
                            if ev.paths.len() == 2 {
                                let from = crate::util::normalize_path(&ev.paths[0]);
                                let to = crate::util::normalize_path(&ev.paths[1]);
                                apply_rename(&state, &from, &to);
                            }
                        }
                        notify::event::EventKind::Remove(_) => {
                            pending.push(p);
                        }
                        _ => {
                            pending.push(p);
                        }
                    }
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if !pending.is_empty() {
                    let drained: Vec<String> = pending.drain(..).collect();
                    apply_paths(&state, drained);
                }
                let _ = rename_from.take();
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
        if !pending.is_empty() && pending.len() > 512 {
            let drained: Vec<String> = pending.drain(..).collect();
            apply_paths(&state, drained);
        }
    }
}

fn apply_paths(state: &Arc<crate::AppState>, paths: Vec<String>) {
    state.db.with_conn(|c| {
        c.execute("BEGIN IMMEDIATE", []).ok();
        for p in &paths {
            if std::path::Path::new(p).exists() {
                let opts_row: Option<(i64, String)> = c
                    .query_row(
                        "SELECT scope_id, options_json FROM scopes WHERE enabled=1 AND (?1 LIKE path || '%') ORDER BY LENGTH(path) DESC LIMIT 1",
                        params![p],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .ok();
                if let Some((scope_id, opts_json)) = opts_row {
                    let opts = scope_options(&opts_json);
                    if let Some(mut u) = crate::scanner::stat_entry(std::path::Path::new(p), scope_id, &opts) {
                        let _ = crate::scanner::upsert_batch(c, std::slice::from_ref(&mut u));
                    }
                }
            } else {
                // 目录删除时清理整个子树
                let _ = c.execute("DELETE FROM files WHERE path=?1", [p]);
                let _ = c.execute("DELETE FROM files WHERE path LIKE ?1", [format!("{p}\\%")]);
                let _ = c.execute(
                    "INSERT INTO index_outbox(file_id, operation, created_ms)
                     SELECT file_id, 'delete', ?2 FROM files WHERE path=?1 OR path LIKE ?3",
                    params![p, crate::util::now_ms(), format!("{p}\\%")],
                );
            }
        }
        c.execute("COMMIT", []).ok();
        Ok::<(), String>(())
    })
    .ok();
}

fn apply_rename(state: &Arc<crate::AppState>, from: &str, to: &str) {
    state.db.with_conn(|c| {
        // 同目录重命名直接改路径；目录重命名处理子树前缀。
        c.execute("UPDATE files SET path=?2, name=?3, updated_ms=?4 WHERE path=?1",
            params![from, to, std::path::Path::new(to).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(), crate::util::now_ms()])
            .ok();
        c.execute("UPDATE files SET path=?2 || SUBSTR(path, LENGTH(?1)+1), updated_ms=?3 WHERE path LIKE ?4",
            params![from, to, crate::util::now_ms(), format!("{from}\\%")])
            .ok();
        let _ = c.execute(
            "INSERT INTO index_outbox(file_id, operation, created_ms) SELECT file_id, 'upsert', ?2 FROM files WHERE path LIKE ?3 OR path=?1",
            params![to, crate::util::now_ms(), format!("{to}\\%")],
        );
        Ok::<(), String>(())
    })
    .ok();
}

fn schedule_calibration(state: &Arc<crate::AppState>) {
    let scope_id: Option<i64> = state.db.with_conn(|c| {
        c.query_row(
            "SELECT scope_id FROM scopes WHERE enabled=1 ORDER BY scope_id LIMIT 1",
            [],
            |r| r.get(0),
        )
        .ok()
    });
    if let Some(sid) = scope_id {
        let _ = crate::scanner::start_recalibrate_job(&state, sid);
    }
}

/// 定期校准线程（默认 30 分钟，设置 calibration.minutes 可调）。
pub fn spawn_calibration_timer(state: Arc<crate::AppState>, stop: Arc<std::sync::atomic::AtomicBool>) {
    std::thread::Builder::new()
        .name("calibration-timer".into())
        .spawn(move || loop {
            if stop.load(std::sync::atomic::Ordering::SeqCst) {
                break;
            }
            std::thread::sleep(Duration::from_secs(30 * 60));
            let scopes: Vec<i64> = state
                .db
                .with_conn(|c| {
                    let mut stmt = c
                        .prepare("SELECT scope_id FROM scopes WHERE enabled=1")
                        .map_err(|e| e.to_string())?;
                    let rows = stmt
                        .query_map([], |r| r.get::<_, i64>(0))
                        .map_err(|e| e.to_string())?;
                    let all: Vec<i64> = rows.filter_map(|x| x.ok()).collect();
                    Ok::<_, String>(all)
                })
                .unwrap_or_default();
            for sid in scopes {
                let _ = crate::scanner::start_recalibrate_job(&state, sid);
            }
        })
        .ok();
}
