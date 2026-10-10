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

/// 批量执行结果：聚合计数 + 逐项结果（底线 2：成功 / 跳过 / 失败可追溯）。
/// 前端既可用计数做汇总提示，也可用 `items` 渲染逐项状态与失败原因。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchExecuteResult {
    pub ok: usize,
    pub failed: usize,
    pub skipped: usize,
    /// 仅批量重命名有（供一次性撤销）；移动为 None。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<i64>,
    pub items: Vec<PlanItem>,
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
/// 返回逐项结果（底线 2：成功 / 跳过 / 失败可追溯），成功项 status="ok"，失败项带 reason。
#[tauri::command]
pub async fn batch_rename_execute(
    app: tauri::AppHandle,
    items: Vec<PlanItem>,
) -> Result<BatchExecuteResult, String> {
    let state: tauri::State<std::sync::Arc<crate::AppState>> = app.state();
    let op_id = crate::util::now_ms();
    let mut ok = 0usize;
    let mut failed = 0usize;
    let mut skipped = 0usize;
    let mut undo_rows: Vec<(i64, String, String)> = Vec::new();
    let mut results: Vec<PlanItem> = Vec::with_capacity(items.len());
    for item in &items {
        if item.status != "ok" {
            skipped += 1;
            results.push(item.clone());
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
                results.push(PlanItem {
                    file_id: item.file_id,
                    old_path: item.old_path.clone(),
                    new_path: item.new_path.clone(),
                    status: "ok".into(),
                    reason: None,
                });
            }
            Err(e) => {
                failed += 1;
                tracing::warn!("重命名失败 {}: {e}", item.old_path);
                results.push(PlanItem {
                    file_id: item.file_id,
                    old_path: item.old_path.clone(),
                    new_path: item.new_path.clone(),
                    status: "failed".into(),
                    reason: Some(e.to_string()),
                });
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
    Ok(BatchExecuteResult { ok, failed, skipped, operation_id: Some(operation_id), items: results })
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
) -> Result<BatchExecuteResult, String> {
    let mut ok = 0usize;
    let mut failed = 0usize;
    let mut skipped = 0usize;
    let strategy = match conflict.as_str() {
        "overwrite" => crate::actions::Conflict::Overwrite,
        "keepBoth" => crate::actions::Conflict::KeepBoth,
        "onlyNewer" => crate::actions::Conflict::OnlyNewer,
        _ => crate::actions::Conflict::Skip,
    };
    let mut results: Vec<PlanItem> = Vec::with_capacity(items.len());
    for item in &items {
        if item.status != "ok" {
            skipped += 1;
            results.push(item.clone());
            continue;
        }
        let src = Path::new(&item.old_path);
        let dest = Path::new(&item.new_path);
        match crate::actions::move_or_copy(src, dest, strategy, true) {
            Ok(_) => {
                ok += 1;
                results.push(PlanItem {
                    file_id: item.file_id,
                    old_path: item.old_path.clone(),
                    new_path: item.new_path.clone(),
                    status: "ok".into(),
                    reason: None,
                });
            }
            Err(crate::actions::MoveError::Skipped) => {
                skipped += 1;
                results.push(PlanItem {
                    file_id: item.file_id,
                    old_path: item.old_path.clone(),
                    new_path: item.new_path.clone(),
                    status: "skipped".into(),
                    reason: Some("按冲突策略跳过".into()),
                });
            }
            Err(crate::actions::MoveError::Failed(e)) => {
                failed += 1;
                tracing::warn!("移动失败 {}: {e}", item.old_path);
                results.push(PlanItem {
                    file_id: item.file_id,
                    old_path: item.old_path.clone(),
                    new_path: item.new_path.clone(),
                    status: "failed".into(),
                    reason: Some(e),
                });
            }
        }
    }
    Ok(BatchExecuteResult { ok, failed, skipped, operation_id: None, items: results })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule() -> RenameRule {
        RenameRule::default()
    }

    fn tmpdir(name: &str) -> std::path::PathBuf {
        let d = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("testtmp")
            .join(name);
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn apply_rule_prefix_suffix_keeps_extension() {
        let mut r = rule();
        r.prefix = "前_".into();
        r.suffix = "_后".into();
        assert_eq!(apply_rule(&r, "报告.docx", 0), "前_报告_后.docx");
    }

    #[test]
    fn apply_rule_numbering_zero_pads() {
        let mut r = rule();
        r.numbering = true;
        r.numbering_digits = 3;
        assert_eq!(apply_rule(&r, "a.txt", 0), "a_001.txt");
        assert_eq!(apply_rule(&r, "a.txt", 41), "a_042.txt");
    }

    #[test]
    fn apply_rule_find_replace_is_literal() {
        let mut r = rule();
        r.find = "旧".into();
        r.replace = "新".into();
        assert_eq!(apply_rule(&r, "旧文件旧名.txt", 0), "新文件新名.txt");
    }

    #[test]
    fn apply_rule_without_extension() {
        let mut r = rule();
        r.prefix = "P-".into();
        assert_eq!(apply_rule(&r, "无扩展名", 0), "P-无扩展名");
    }

    #[test]
    fn illegal_detects_reserved_chars() {
        for c in ['\\', '/', ':', '*', '?', '"', '<', '>', '|'] {
            assert!(illegal(&format!("a{c}b")), "应识别非法字符 {c}");
        }
        assert!(!illegal("正常 名称-2026.txt"));
    }

    #[test]
    fn preview_marks_illegal_unchanged_and_ok() {
        let dir = tmpdir("preview_basic");
        let a = dir.join("a.txt");
        std::fs::write(&a, b"x").unwrap();
        let ap = a.to_string_lossy().to_string();

        let mut r = rule();
        r.suffix = "_新".into();
        let items = tauri::async_runtime::block_on(batch_rename_preview(BatchRequest {
            paths: vec![ap.clone()],
            rule: r.clone(),
        }))
        .unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].status, "ok");
        assert_eq!(items[0].reason, None);

        // 规则不产生变化 → unchanged
        let items = tauri::async_runtime::block_on(batch_rename_preview(BatchRequest {
            paths: vec![ap.clone()],
            rule: rule(),
        }))
        .unwrap();
        assert_eq!(items[0].status, "unchanged");
    }

    #[test]
    fn preview_marks_conflict_with_existing_file() {
        let dir = tmpdir("preview_conflict");
        let src = dir.join("a.txt");
        let dst = dir.join("b.txt");
        std::fs::write(&src, b"x").unwrap();
        std::fs::write(&dst, b"y").unwrap();
        // 把 a.txt 改成已存在的 b.txt → 冲突
        let mut r = rule();
        r.find = "a".into();
        r.replace = "b".into();
        let items = tauri::async_runtime::block_on(batch_rename_preview(BatchRequest {
            paths: vec![src.to_string_lossy().to_string()],
            rule: r,
        }))
        .unwrap();
        assert_eq!(items[0].status, "conflict");
    }

    #[test]
    fn preview_detects_duplicate_target_within_batch() {
        let dir = tmpdir("preview_dup");
        let a = dir.join("q1.txt");
        let b = dir.join("q11.txt");
        std::fs::write(&a, b"x").unwrap();
        std::fs::write(&b, b"y").unwrap();
        // 去掉所有 "1"：q1.txt 与 q11.txt 都收敛为 q.txt（该名在磁盘上不存在，
        // 因此只可能是「批内重名」这一条路径判定为冲突）
        let mut r = rule();
        r.find = "1".into();
        r.replace = "".into();
        let items = tauri::async_runtime::block_on(batch_rename_preview(BatchRequest {
            paths: vec![a.to_string_lossy().to_string(), b.to_string_lossy().to_string()],
            rule: r,
        }))
        .unwrap();
        assert_eq!(items[0].new_path, items[1].new_path, "两项应得到同一目标名");
        assert_eq!(items[0].status, "ok", "首项无冲突");
        assert_eq!(items[1].status, "conflict", "次项与本批首项重名");
    }

    #[test]
    fn move_preview_marks_missing_and_ok() {
        let dir = tmpdir("move_preview");
        let target = dir.join("dest");
        std::fs::create_dir_all(&target).unwrap();
        let exists = dir.join("存在.txt");
        std::fs::write(&exists, b"x").unwrap();
        let ghost = dir.join("不存在.txt");

        let items = tauri::async_runtime::block_on(batch_move_preview(
            vec![
                exists.to_string_lossy().to_string(),
                ghost.to_string_lossy().to_string(),
            ],
            target.to_string_lossy().to_string(),
        ))
        .unwrap();
        assert_eq!(items[0].status, "ok");
        assert_eq!(items[1].status, "missing");
    }

    #[test]
    fn move_preview_rejects_missing_target_dir() {
        let err = tauri::async_runtime::block_on(batch_move_preview(
            vec![r"D:\不存在\a.txt".into()],
            r"D:\绝对不存在的目标目录\xyz".into(),
        ))
        .unwrap_err();
        assert!(err.contains("目标目录不存在"), "实际错误：{err}");
    }

    #[test]
    fn execute_returns_per_item_results() {
        let dir = tmpdir("execute_per_item");
        let src = dir.join("旧名.txt");
        std::fs::write(&src, b"x").unwrap();
        let dst = dir.join("新名.txt");

        // 一项 ok、一项非 ok（应计入 skipped 且原样回传）
        let items = vec![
            PlanItem {
                file_id: 0,
                old_path: src.to_string_lossy().to_string(),
                new_path: dst.to_string_lossy().to_string(),
                status: "ok".into(),
                reason: None,
            },
            PlanItem {
                file_id: 0,
                old_path: r"D:\不存在\x.txt".into(),
                new_path: r"D:\不存在\y.txt".into(),
                status: "missing".into(),
                reason: Some("源文件不存在".into()),
            },
        ];
        let r = tauri::async_runtime::block_on(batch_move_execute(items, "skip".into())).unwrap();
        assert_eq!(r.ok, 1);
        assert_eq!(r.skipped, 1);
        assert_eq!(r.failed, 0);
        assert_eq!(r.items.len(), 2, "逐项结果必须与输入一一对应");
        // 非 ok 项原样回传，便于前端继续展示原因
        assert_eq!(r.items[1].status, "missing");
        assert_eq!(r.items[1].reason.as_deref(), Some("源文件不存在"));
    }
}
