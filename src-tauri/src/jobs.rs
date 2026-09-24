use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;

use crate::settings::SettingsStore;
use crate::util;

pub struct JobFlags {
    pub cancel: AtomicBool,
    pub pause: AtomicBool,
}

/// 后台任务引擎：状态持久化到 jobs 表，支持暂停 / 恢复 / 取消 / 崩溃恢复（FR03）。
pub struct JobEngine {
    pub db: Arc<SettingsStore>,
    handles: Arc<Mutex<HashMap<i64, Arc<JobFlags>>>>,
}

impl JobEngine {
    pub fn new(db: Arc<SettingsStore>) -> Self {
        Self {
            db,
            handles: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 启动恢复：把上次运行中的任务标记为 Interrupted（设计文档 §12.2）。
    pub fn recover(&self) {
        let _ = self.db.with_conn(|c| {
            c.execute(
                "UPDATE jobs SET state='interrupted', updated_at=?1
                 WHERE state IN ('waiting','running','paused')",
                [util::now_rfc3339()],
            )
        });
    }

    pub fn start<F>(&self, kind: &str, _payload: Value, body: F) -> Result<i64, String>
    where
        F: FnOnce(JobContext) + Send + 'static,
    {
        // 同类任务去重：同类运行中则拒绝重复启动。
        let dup: i64 = self.db.with_conn(|c| {
            let mut stmt = c
                .prepare("SELECT COUNT(*) FROM jobs WHERE kind=?1 AND state IN ('waiting','running','paused')")
                .map_err(|e| e.to_string())?;
            stmt
                .query_row([kind], |r| r.get::<_, i64>(0))
                .map_err(|e| e.to_string())
        })?;
        if dup > 0 {
            return Err(format!("任务 {kind} 正在运行中"));
        }
        let job_id: i64 = self
            .db
            .with_conn(|c| {
                c.execute(
                    "INSERT INTO jobs(kind, state, created_at, updated_at) VALUES (?1,'waiting',?2,?2)",
                    rusqlite::params![kind, util::now_rfc3339()],
                )
                .map_err(|e| e.to_string())?;
                Ok::<i64, String>(c.last_insert_rowid())
            })
            .map_err(|e| e.to_string())?;
        let flags = Arc::new(JobFlags {
            cancel: AtomicBool::new(false),
            pause: AtomicBool::new(false),
        });
        self.handles.lock().unwrap().insert(job_id, flags.clone());
        let ctx = JobContext {
            job_id,
            db: self.db.clone(),
            flags,
            last_report: Mutex::new(std::time::Instant::now()),
        };
        let handles = self.handles.clone();
        std::thread::Builder::new()
            .name(format!("job-{kind}-{job_id}"))
            .spawn(move || {
                ctx.set_state("running");
                body(ctx);
                handles.lock().unwrap().remove(&job_id);
            })
            .map_err(|e| e.to_string())?;
        Ok(job_id)
    }

    pub fn cancel(&self, job_id: i64) -> Result<(), String> {
        let flags = self.handles.lock().unwrap().get(&job_id).cloned();
        match flags {
            Some(f) => {
                f.cancel.store(true, Ordering::SeqCst);
                Ok(())
            }
            None => Err("任务不在运行中".into()),
        }
    }

    pub fn pause(&self, job_id: i64) -> Result<(), String> {
        let flags = self.handles.lock().unwrap().get(&job_id).cloned();
        match flags {
            Some(f) => {
                f.pause.store(true, Ordering::SeqCst);
                self.db
                    .with_conn(|c| {
                        c.execute(
                            "UPDATE jobs SET state='paused', updated_at=?2 WHERE job_id=?1",
                            rusqlite::params![job_id, util::now_rfc3339()],
                        )
                    })
                    .map_err(|e| e.to_string())?;
                Ok(())
            }
            None => Err("任务不在运行中".into()),
        }
    }

    pub fn resume(&self, job_id: i64) -> Result<(), String> {
        let flags = self.handles.lock().unwrap().get(&job_id).cloned();
        match flags {
            Some(f) => {
                f.pause.store(false, Ordering::SeqCst);
                self.db
                    .with_conn(|c| {
                        c.execute(
                            "UPDATE jobs SET state='running', updated_at=?2 WHERE job_id=?1",
                            rusqlite::params![job_id, util::now_rfc3339()],
                        )
                    })
                    .map_err(|e| e.to_string())?;
                Ok(())
            }
            None => Err("任务不在运行中".into()),
        }
    }
}

pub struct JobContext {
    pub job_id: i64,
    pub db: Arc<SettingsStore>,
    flags: Arc<JobFlags>,
    last_report: Mutex<std::time::Instant>,
}

impl JobContext {
    pub fn cancelled(&self) -> bool {
        self.flags.cancel.load(Ordering::SeqCst)
    }

    pub fn wait_if_paused(&self) {
        while self.flags.pause.load(Ordering::SeqCst) && !self.flags.cancel.load(Ordering::SeqCst)
        {
            std::thread::sleep(Duration::from_millis(200));
        }
    }

    /// 汇报进度（写库节流到 200ms，避免小任务放大写放大）。
    pub fn progress(&self, p: f64, message: &str) {
        let mut last = self.last_report.lock().unwrap();
        if last.elapsed() < Duration::from_millis(200) {
            return;
        }
        *last = std::time::Instant::now();
        let _ = self.db.with_conn(|c| {
            c.execute(
                "UPDATE jobs SET progress=?2, message=?3, updated_at=?4 WHERE job_id=?1",
                rusqlite::params![
                    self.job_id,
                    p.clamp(0.0, 0.99),
                    message,
                    util::now_rfc3339()
                ],
            )
        });
    }

    pub fn finish_ok(&self, result_json: Option<String>) {
        self.db
            .with_conn(|c| {
                c.execute(
                    "UPDATE jobs SET state='completed', progress=1.0, result_json=?2, updated_at=?3 WHERE job_id=?1",
                    rusqlite::params![self.job_id, result_json, util::now_rfc3339()],
                )
            })
            .ok();
    }

    pub fn finish_cancelled(&self) {
        self.set_state("cancelled");
    }

    pub fn finish_failed(&self, error: &str) {
        self.db
            .with_conn(|c| {
                c.execute(
                    "UPDATE jobs SET state='failed', result_json=?2, updated_at=?3 WHERE job_id=?1",
                    rusqlite::params![
                        self.job_id,
                        &serde_json::json!({"error": error}).to_string(),
                        util::now_rfc3339()
                    ],
                )
            })
            .ok();
    }

    pub fn set_state(&self, state: &str) {
        self.db
            .with_conn(|c| {
                c.execute(
                    "UPDATE jobs SET state=?2, updated_at=?3 WHERE job_id=?1",
                    rusqlite::params![self.job_id, state, util::now_rfc3339()],
                )
            })
            .ok();
    }

    /// 崩溃可恢复的检查点（设计文档 §12.2）。
    pub fn checkpoint(&self, checkpoint_json: &str) {
        self.db
            .with_conn(|c| {
                c.execute(
                    "UPDATE jobs SET checkpoint_json=?2, updated_at=?3 WHERE job_id=?1",
                    rusqlite::params![self.job_id, checkpoint_json, util::now_rfc3339()],
                )
            })
            .ok();
    }
}

#[tauri::command]
pub fn jobs_list(store: tauri::State<'_, std::sync::Arc<crate::AppState>>, limit: i64) -> Result<Vec<crate::models::JobInfo>, String> {
    store.engine.db.with_conn(|c| {
        let mut stmt = c
            .prepare(
                "SELECT job_id, kind, state, progress, message, result_json, created_at, updated_at
                 FROM jobs ORDER BY job_id DESC LIMIT ?1",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([limit], |r| {
                Ok(crate::models::JobInfo {
                    job_id: r.get(0)?,
                    kind: r.get(1)?,
                    state: r.get(2)?,
                    progress: r.get(3)?,
                    message: r.get(4)?,
                    result_json: r.get(5)?,
                    created_at: r.get(6)?,
                    updated_at: r.get(7)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn job_pause(store: tauri::State<'_, std::sync::Arc<crate::AppState>>, job_id: i64) -> Result<(), String> {
    store.engine.pause(job_id)
}

#[tauri::command]
pub fn job_resume(store: tauri::State<'_, std::sync::Arc<crate::AppState>>, job_id: i64) -> Result<(), String> {
    store.engine.resume(job_id)
}

#[tauri::command]
pub fn job_cancel(store: tauri::State<'_, std::sync::Arc<crate::AppState>>, job_id: i64) -> Result<(), String> {
    store.engine.cancel(job_id)
}

#[tauri::command]
pub fn job_retry_failed(store: tauri::State<'_, std::sync::Arc<crate::AppState>>, job_id: i64) -> Result<(), String> {
    // 简单重试：把失败任务重新入队（重建同 kind 任务，由各任务自行读取 checkpoint 续跑）。
    let kind: String = store
        .engine
        .db
        .with_conn(|c| {
            c.query_row(
                "SELECT kind FROM jobs WHERE job_id=?1",
                [job_id],
                |r| r.get(0),
            )
        })
        .map_err(|_| "任务不存在".to_string())?;
    if kind == "initial_scan" {
        let scope_id: i64 = store
            .engine
            .db
            .with_conn(|c| {
                c.query_row(
                    "SELECT json_extract(result_json,'$.scopeId') FROM jobs WHERE job_id=?1",
                    [job_id],
                    |r| r.get(0),
                )
            })
            .unwrap_or(0);
        return crate::scanner::start_scan_job(&store, scope_id);
    }
    Err("该任务类型暂不支持自动重试".into())
}
