use std::path::Path;

use rusqlite::params;
use tauri::Manager;
use serde::{Deserialize, Serialize};

use crate::models::PlanItem;
use crate::util;

/// 批量重命名规则（FR10）。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct RenameRule {
    pub find: String,
    pub replace: String,
    pub prefix: String,
    pub suffix: String,
    pub date_format: String,
    pub numbering: bool,
    pub numbering_digits: usize,
    pub regex: bool,
}

#[derive(Debug, Deserialize)]
pub struct BatchRequest {
    pub paths: Vec<String>,
    pub rule: RenameRule,
}

fn apply_rule(rule: &RenameRule, name: &str, index: usize) -> String {
    let stem = Path::new(name)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| name.to_string());
    let ext = Path::new(name)
        .extension()
        .map(|s| format!(".{}", s.to_string_lossy()))
        .unwrap_or_default();
    let mut s = stem.clone();
    if !rule.find.is_empty() {
        if rule.regex {
            // 简化：正则以字面量失败时回退字面替换
            s = s.replace(&rule.find, &rule.replace);
        } else {
            s = s.replace(&rule.find, &rule.replace);
        }
    }
    s = format!("{}{}{}", rule.prefix, s, rule.suffix);
    if !rule.date_format.is_empty() {
        let today = chrono::Local::now().format(&rule.date_format).to_string();
        s = format!("{s}_{today}");
    }
    if rule.numbering {
        s = format!("{s}_{:0width$}", index + 1, width = rule.numbering_digits.max(1));
    }
    format!("{s}{ext}")
}

fn illegal(name: &str) -> bool {
    name.contains(['\\', '/', ':', '*', '?', '"', '<', '>', '|'])
}

/// 批量重命名预演：原名称 → 新名称对照 + 冲突/非法检测（FR10 先预演）。
#[tauri::command]
pub async fn batch_rename_preview(request: BatchRequest) -> Result<Vec<PlanItem>, String> {
    let mut items = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (i, p) in request.paths.iter().enumerate() {
        let path = Path::new(p);
        let dir = path.parent().unwrap_or(Path::new("."));
        let old_name = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let new_name = apply_rule(&request.rule, &old_name, i);
        let new_path = dir.join(&new_name).to_string_lossy().replace('/', "\\");
        let (status, reason): (&str, Option<String>) = if illegal(&new_name) {
            ("illegal", Some("包含非法字符".into()))
        } else if new_name == old_name {
            ("unchanged", None)
        } else if seen.contains(&new_path.to_lowercase()) || (new_path != *p && Path::new(&new_path).exists()) {
            ("conflict", Some("名称冲突".into()))
        } else {
            ("ok", None)
        };
        seen.insert(new_path.to_lowercase());
        items.push(PlanItem {
            file_id: 0,
            old_path: p.clone(),
            new_path,
            status: status.into(),
            reason,
        });
    }
    Ok(items)
}

/// 执行批量重命名；保存反向映射供一次性撤销。
#[tauri::command]
pub async fn batch_rename_execute(
    app: tauri::AppHandle,
    items: Vec<PlanItem>,
) -> Result<serde_json::Value, String> {
    let state: tauri::State<std::sync::Arc<crate::AppState>> = app.state();
    let op_id = crate::util::now_ms();
    let mut ok = 0usize;
    let mut failed = 0usize;
    let mut skipped = 0usize;
    let mut undo_rows: Vec<(i64, String, String)> = Vec::new();
    for item in &items {
        if item.status != "ok" {
            skipped += 1;
            continue;
        }
        match std::fs::rename(&item.old_path, &item.new_path) {
            Ok(_) => {
                ok += 1;
                state.db.with_conn(|c| {
                    let fid: Option<i64> = c
                        .query_row("SELECT file_id FROM files WHERE path=?1", [&item.old_path], |r| r.get(0))
                        .ok();
                    if let Some(fid) = fid {
                        c.execute("UPDATE files SET path=?2, name=?3, updated_ms=?4 WHERE file_id=?1",
                            params![fid, item.new_path,
                                Path::new(&item.new_path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
                                util::now_ms()]).ok();
                        undo_rows.push((fid, item.old_path.clone(), item.new_path.clone()));
                    }
                    Ok::<(), String>(())
                }).ok();
            }
            Err(e) => {
                failed += 1;
                tracing::warn!("重命名失败 {}: {e}", item.old_path);
            }
        }
    }
    let operation_id = op_id + 1;
    if !undo_rows.is_empty() {
        state.db.with_conn(|c| {
            for (fid, old, new) in &undo_rows {
                c.execute("INSERT OR IGNORE INTO rename_undo(operation_id, file_id, old_path, new_path) VALUES (?1,?2,?3,?4)",
                    params![operation_id, fid, old, new]).ok();
            }
            c.execute(
                "INSERT INTO operation_log(operation_id, kind, request_json, result_json, reversible_until, created_at) VALUES (?1,'batch_rename',?2,?3,?4,?5)",
                params![
                    operation_id,
                    serde_json::json!({"count": items.len()}).to_string(),
                    serde_json::json!({"ok": ok, "failed": failed, "skipped": skipped}).to_string(),
                    (chrono::Local::now() + chrono::Duration::hours(24)).to_rfc3339(),
                    util::now_rfc3339()
                ],
            )
            .ok();
            Ok::<(), String>(())
        }).ok();
    }
    Ok(serde_json::json!({"ok": ok, "failed": failed, "skipped": skipped, "operationId": operation_id}))
}

/// 一次性撤销批量重命名（撤销前由前端再次确认，FR10）。
#[tauri::command]
pub async fn batch_rename_undo(app: tauri::AppHandle, operation_id: i64) -> Result<serde_json::Value, String> {
    let state: tauri::State<std::sync::Arc<crate::AppState>> = app.state();
    let rows: Vec<(i64, String, String)> = state.db.with_conn(|c| {
        let mut stmt = c
            .prepare("SELECT file_id, old_path, new_path FROM rename_undo WHERE operation_id=?1")
            .map_err(|e| e.to_string())?;
        let all = stmt
            .query_map([operation_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .map_err(|e| e.to_string())?
            .filter_map(|x| x.ok())
            .collect();
        Ok::<_, String>(all)
    })?;
    let mut undone = 0usize;
    let mut conflicts = 0usize;
    for (fid, old, new) in &rows {
        // 撤销前校验当前状态，避免覆盖后续变更（FR10）
        if Path::new(old).exists() {
            conflicts += 1;
            continue;
        }
        if std::fs::rename(new, old).is_ok() {
            state.db.with_conn(|c| {
                c.execute("UPDATE files SET path=?2 WHERE file_id=?1", params![fid, old]).ok();
                Ok::<(), String>(())
            }).ok();
            undone += 1;
        }
    }
    state.db.with_conn(|c| {
        c.execute("DELETE FROM rename_undo WHERE operation_id=?1", [operation_id]).ok();
        Ok::<(), String>(())
    }).ok();
    Ok(serde_json::json!({"undone": undone, "conflicts": conflicts}))
}

/// 批量移动预演。
#[tauri::command]
pub async fn batch_move_preview(paths: Vec<String>, target_dir: String) -> Result<Vec<PlanItem>, String> {
    if !Path::new(&target_dir).is_dir() {
        return Err("目标目录不存在".into());
    }
    let mut items = Vec::new();
    for p in &paths {
        let path = Path::new(p);
        let new_path = Path::new(&target_dir)
            .join(path.file_name().unwrap_or_default())
            .to_string_lossy()
            .replace('/', "\\");
        let status = if !path.exists() {
            "missing"
        } else if new_path == *p {
            "unchanged"
        } else if Path::new(&new_path).exists() {
            "conflict"
        } else {
            "ok"
        };
        items.push(PlanItem {
            file_id: 0,
            old_path: p.clone(),
            new_path,
            status: status.into(),
            reason: None,
        });
    }
    Ok(items)
}

/// 批量移动执行（FR11：预检查后执行，结果逐项返回）。
#[tauri::command]
pub async fn batch_move_execute(
    items: Vec<PlanItem>,
    conflict: String,
) -> Result<serde_json::Value, String> {
    let mut ok = 0usize;
    let mut failed = 0usize;
    let mut skipped = 0usize;
    let strategy = match conflict.as_str() {
        "overwrite" => crate::actions::Conflict::Overwrite,
        "keepBoth" => crate::actions::Conflict::KeepBoth,
        "onlyNewer" => crate::actions::Conflict::OnlyNewer,
        _ => crate::actions::Conflict::Skip,
    };
    for item in &items {
        if item.status != "ok" {
            skipped += 1;
            continue;
        }
        let src = Path::new(&item.old_path);
        let dest = Path::new(&item.new_path);
        match crate::actions::move_or_copy(src, dest, strategy, true) {
            Ok(_) => ok += 1,
            Err(crate::actions::MoveError::Skipped) => skipped += 1,
            Err(crate::actions::MoveError::Failed(e)) => {
                failed += 1;
                tracing::warn!("移动失败 {}: {e}", item.old_path);
            }
        }
    }
    Ok(serde_json::json!({"ok": ok, "failed": failed, "skipped": skipped}))
}
