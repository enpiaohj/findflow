use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use notify::{RecursiveMode, Watcher};
use rusqlite::{params, Connection};
use std::path::Path;

use crate::scope::{scope_options, ScopeOptions};

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
    // Windows 下重命名拆成 From / To 两个相邻事件，这里配对后按重命名处理
    let mut rename_from: Option<String> = None;
    loop {
        match rx.recv_timeout(Duration::from_millis(250)) {
            Ok(WatchMsg::Event(ev)) => {
                if ev.need_rescan() {
                    // 缓冲区溢出：调度全量校准
                    schedule_calibration(&state);
                    continue;
                }
                use notify::event::{EventKind, ModifyKind, RenameMode};
                let paths: Vec<String> = ev
                    .paths
                    .iter()
                    .map(|p| crate::util::normalize_path(p))
                    .collect();
                match ev.kind {
                    EventKind::Modify(ModifyKind::Name(RenameMode::From)) => {
                        if let Some(prev) = rename_from.take() {
                            pending.push(prev);
                        }
                        rename_from = paths.into_iter().next();
                    }
                    EventKind::Modify(ModifyKind::Name(RenameMode::To)) => {
                        for to in paths {
                            match rename_from.take() {
                                Some(from) => {
                                    // 先落地之前的事件，保证重命名看到的是最新库状态
                                    flush(&state, &mut pending);
                                    apply_rename(&state, &from, &to);
                                }
                                None => pending.push(to),
                            }
                        }
                    }
                    EventKind::Modify(ModifyKind::Name(RenameMode::Both)) if paths.len() == 2 => {
                        flush(&state, &mut pending);
                        apply_rename(&state, &paths[0], &paths[1]);
                    }
                    _ => pending.extend(paths),
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // 只有 From 没有 To：移出了监听范围，按删除处理
                if let Some(from) = rename_from.take() {
                    pending.push(from);
                }
                flush(&state, &mut pending);
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
        if pending.len() > 512 {
            flush(&state, &mut pending);
        }
    }
}

fn flush(state: &Arc<crate::AppState>, pending: &mut Vec<String>) {
    if pending.is_empty() {
        return;
    }
    let mut drained: Vec<String> = std::mem::take(pending);
    // 同一窗口内同一路径的多次事件只处理一次（最终以磁盘当前状态为准）
    drained.sort_unstable();
    drained.dedup();
    // 应用自身数据目录（数据库 / 索引 / 日志）的变化必须忽略，否则会自我触发无限循环
    drained.retain(|p| !crate::util::is_app_data_path(p));
    if !drained.is_empty() {
        apply_paths(state, drained);
    }
}

/// 查找路径所属的启用范围（最长前缀匹配，且前缀必须落在路径分隔符边界上）。
fn scope_for(c: &Connection, p: &str) -> Option<(i64, String, String)> {
    c.prepare_cached(
        "SELECT scope_id, path, options_json FROM scopes
         WHERE enabled=1
           AND (?1 = path
                OR ?1 LIKE (CASE WHEN substr(path, -1) = '\\' THEN path ELSE path || '\\' END) || '%')
         ORDER BY LENGTH(path) DESC LIMIT 1",
    )
    .and_then(|mut s| s.query_row([p], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))))
    .ok()
}

/// 路径相对范围根的各级目录/文件名是否命中排除规则、受保护目录或 `$` 系统目录
/// （与全量扫描的整树跳过规则保持一致）。
fn excluded_by_rules(p: &str, scope_path: &str, opts: &ScopeOptions) -> bool {
    let rel = p.get(scope_path.len()..).unwrap_or("");
    rel.split('\\').filter(|s| !s.is_empty()).any(|seg| {
        seg.starts_with('$')
            || crate::scope::is_protected(seg)
            || crate::scope::segment_excluded(seg, opts)
    })
}

/// 删除路径本身及其子树的行，并先登记索引删除事件（先登记再删行，否则事件查不到行）。
fn delete_subtree(c: &Connection, p: &str) {
    let (lo, hi) = crate::util::subtree_bounds(p);
    let _ = c.execute(
        "INSERT INTO index_outbox(file_id, operation, created_ms)
         SELECT file_id, 'delete', ?4 FROM files WHERE path=?1 OR (path>=?2 AND path<?3)",
        params![p, lo, hi, crate::util::now_ms()],
    );
    let _ = c.execute(
        "DELETE FROM files WHERE path=?1 OR (path>=?2 AND path<?3)",
        params![p, lo, hi],
    );
}

fn apply_paths(state: &Arc<crate::AppState>, paths: Vec<String>) {
    state.bg_db.with_conn(|c| {
        c.execute("BEGIN IMMEDIATE", []).ok();
        for p in &paths {
            match std::fs::symlink_metadata(p) {
                Ok(_) => {
                    let Some((scope_id, scope_path, opts_json)) = scope_for(c, p) else {
                        continue;
                    };
                    let opts = scope_options(&opts_json);
                    if excluded_by_rules(p, &scope_path, &opts) {
                        continue;
                    }
                    if let Some(u) = crate::scanner::stat_entry(Path::new(p), scope_id, &opts) {
                        let _ = crate::scanner::upsert_batch(c, std::slice::from_ref(&u));
                    }
                }
                // 仅在确认不存在时删除；无权限等暂时性错误不得误删
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => delete_subtree(c, p),
                Err(_) => {}
            }
        }
        c.execute("COMMIT", []).ok();
    });
}

fn apply_rename(state: &Arc<crate::AppState>, from: &str, to: &str) {
    if crate::util::is_app_data_path(from) && crate::util::is_app_data_path(to) {
        return;
    }
    let handled = state.bg_db.with_conn(|c| {
        let excluded = crate::util::is_app_data_path(to)
            || match scope_for(c, to) {
                Some((_, scope_path, opts_json)) => {
                    excluded_by_rules(to, &scope_path, &scope_options(&opts_json))
                }
                None => true,
            };
        if excluded {
            return false;
        }
        let name = Path::new(to)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let parent = Path::new(to)
            .parent()
            .map(crate::util::normalize_path)
            .unwrap_or_default();
        let now = crate::util::now_ms();
        c.execute("BEGIN IMMEDIATE", []).ok();
        // 目标路径已有行（覆盖式重命名）时唯一约束失败，回退为删除 + 新增
        let moved = c
            .execute(
                "UPDATE files SET path=?2, name=?3, parent_path=?4, updated_ms=?5 WHERE path=?1",
                params![from, to, name, parent, now],
            )
            .unwrap_or(0);
        if moved == 0 {
            c.execute("ROLLBACK", []).ok();
            return false;
        }
        // 目录重命名：子树路径与父路径整体替换前缀（子项文件名不变，无需重建索引文档）
        let (lo, hi) = crate::util::subtree_bounds(from);
        let children_ok = c
            .execute(
                "UPDATE files SET path = ?1 || substr(path, length(?2) + 1),
                                  parent_path = ?1 || substr(parent_path, length(?2) + 1),
                                  updated_ms = ?5
                 WHERE path>=?3 AND path<?4",
                params![to, from, lo, hi, now],
            )
            .is_ok();
        if !children_ok {
            c.execute("ROLLBACK", []).ok();
            return false;
        }
        let _ = c.execute(
            "INSERT INTO index_outbox(file_id, operation, created_ms)
             SELECT file_id, 'upsert', ?2 FROM files WHERE path=?1",
            params![to, now],
        );
        c.execute("COMMIT", []).ok();
        true
    });
    if !handled {
        // 原路径不在库中（如临时文件改名为正式文件）、目标被排除或冲突：按两端实际状态处理
        let mut pending = vec![from.to_string(), to.to_string()];
        flush(state, &mut pending);
    }
}

fn schedule_calibration(state: &Arc<crate::AppState>) {
    let _ = crate::scanner::start_recalibrate_job(state, None);
}

/// 定期校准线程（默认 30 分钟，设置 calibration.minutes 可调，最小 5 分钟）。
/// 已有扫描或校准任务运行时顺延到下一周期。
pub fn spawn_calibration_timer(state: Arc<crate::AppState>, stop: Arc<std::sync::atomic::AtomicBool>) {
    std::thread::Builder::new()
        .name("calibration-timer".into())
        .spawn(move || {
            let mut last_run = Instant::now();
            loop {
                if stop.load(std::sync::atomic::Ordering::SeqCst) {
                    break;
                }
                std::thread::sleep(Duration::from_secs(60));
                let minutes = state
                    .db
                    .get_or("calibration.minutes", serde_json::json!(30))
                    .as_u64()
                    .unwrap_or(30)
                    .max(5);
                if last_run.elapsed() < Duration::from_secs(minutes * 60) {
                    continue;
                }
                let busy: i64 = state.bg_db.with_conn(|c| {
                    c.query_row(
                        "SELECT COUNT(*) FROM jobs
                         WHERE kind IN ('initial_scan','recalibrate')
                           AND state IN ('waiting','running','paused')",
                        [],
                        |r| r.get(0),
                    )
                    .unwrap_or(0)
                });
                if busy > 0 {
                    continue;
                }
                last_run = Instant::now();
                let _ = crate::scanner::start_recalibrate_job(&state, None);
            }
        })
        .ok();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delete_subtree_enqueues_events_and_spares_siblings() {
        let c = crate::db::open_in_memory().unwrap();
        c.execute(
            r"INSERT INTO scopes(scope_id, path, added_at) VALUES (1, 'D:\', 'now')",
            [],
        )
        .unwrap();
        for p in [r"D:\a", r"D:\a\x.txt", r"D:\a\sub\y.txt", r"D:\ab\z.txt", r"D:\a_b"] {
            c.execute(
                "INSERT INTO files(scope_id, name, path, kind) VALUES (1, 'n', ?1, 'doc')",
                [p],
            )
            .unwrap();
        }
        delete_subtree(&c, r"D:\a");
        let left: Vec<String> = c
            .prepare("SELECT path FROM files ORDER BY path")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(|x| x.unwrap())
            .collect();
        assert_eq!(left, vec![r"D:\a_b".to_string(), r"D:\ab\z.txt".to_string()]);
        let deletes: i64 = c
            .query_row(
                "SELECT COUNT(*) FROM index_outbox WHERE operation='delete'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(deletes, 3);
    }

    #[test]
    fn scope_lookup_requires_separator_boundary() {
        let c = crate::db::open_in_memory().unwrap();
        c.execute(
            r"INSERT INTO scopes(scope_id, path, added_at) VALUES (1, 'D:\', 'now'), (2, 'D:\proj', 'now')",
            [],
        )
        .unwrap();
        assert_eq!(scope_for(&c, r"D:\proj\a.txt").map(|s| s.0), Some(2));
        assert_eq!(scope_for(&c, r"D:\project2\a.txt").map(|s| s.0), Some(1));
        assert_eq!(scope_for(&c, r"E:\x").map(|s| s.0), None);
    }

    #[test]
    fn rules_exclude_relative_segments() {
        let opts = ScopeOptions {
            exclude_dirs: vec!["node_modules".into()],
            exclude_exts: vec!["tmp".into()],
            include_hidden: false,
        };
        let root = r"D:\";
        assert!(excluded_by_rules(r"D:\web\node_modules\x.js", root, &opts));
        assert!(excluded_by_rules(r"D:\$RECYCLE.BIN\S-1\a.txt", root, &opts));
        assert!(excluded_by_rules(r"D:\w\a.tmp", root, &opts));
        assert!(!excluded_by_rules(r"D:\web\src\x.js", root, &opts));
    }
}
