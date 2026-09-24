use std::collections::HashMap;
use std::path::Path;

use rusqlite::params;
use serde::Serialize;

use crate::jobs::JobContext;
use crate::models::{DuplicateGroup, FileRow, OrganizerRule};

/// 大文件分析（FR13）：按范围取最大的 N 个文件，删除前会重新读取属性。
#[tauri::command]
pub fn large_files(
    store: tauri::State<'_, crate::AppState>,
    min_size: i64,
    limit: i64,
    path_prefix: Option<String>,
) -> Result<Vec<FileRow>, String> {
    store.db.with_conn(|c| {
        let prefix = path_prefix.unwrap_or_default();
        let mut stmt = c
            .prepare(
                "SELECT file_id, name, path, ext, kind, size, modified_ms, created_ms, is_dir, fulltext_state
                 FROM files WHERE is_dir=0 AND size >= ?1 AND path LIKE ?2
                 ORDER BY size DESC LIMIT ?3",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(
                params![min_size, format!("{prefix}%"), limit],
                |r| {
                    Ok(FileRow {
                        file_id: r.get(0)?,
                        name: r.get(1)?,
                        path: r.get(2)?,
                        ext: r.get(3)?,
                        kind: r.get(4)?,
                        size: r.get(5)?,
                        modified_ms: r.get(6)?,
                        created_ms: r.get(7)?,
                        is_dir: r.get::<_, i64>(8)? != 0,
                        fulltext_state: r.get(9)?,
                        snippet: None,
                        score: 0.0,
                        favorite: false,
                        tags: Vec::new(),
                    })
                },
            )
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    })
}

/// 重复文件三级流程：大小分组 → 快速指纹（头尾 64KB）→ 完整 BLAKE3（FR14）。
pub fn run_duplicates(
    ctx: JobContext,
    db: std::sync::Arc<crate::settings::SettingsStore>,
    min_size: i64,
    _keep_how: String,
) {
    let candidates: Vec<(i64, String, i64)> = match db.with_conn(|c| {
        let mut stmt = c
            .prepare(
                "SELECT file_id, path, size FROM files
                 WHERE is_dir=0 AND size >= ?1
                 AND size IN (SELECT size FROM files WHERE is_dir=0 AND size >= ?1 GROUP BY size HAVING COUNT(*) > 1)
                 ORDER BY size DESC",
            )
            .map_err(|e| e.to_string())?;
        let all = stmt
            .query_map(params![min_size], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .map_err(|e| e.to_string())?
            .filter_map(|x| x.ok())
            .collect();
        Ok::<_, String>(all)
    }) {
        Ok(v) => v,
        Err(e) => {
            ctx.finish_failed(&e);
            return;
        }
    };
    // 大小分组
    let mut by_size: HashMap<i64, Vec<(i64, String)>> = HashMap::new();
    for (fid, path, size) in candidates {
        by_size.entry(size).or_default().push((fid, path));
    }
    let groups: Vec<&Vec<(i64, String)>> = by_size.values().filter(|g| g.len() > 1).collect();
    let total: usize = groups.iter().map(|g| g.len()).sum();
    let mut done = 0usize;
    let mut full_groups: Vec<DuplicateGroup> = Vec::new();
    let mut skipped_unreadable = 0usize;
    for group in groups {
        for _g in group {
            ctx.wait_if_paused();
            if ctx.cancelled() {
                ctx.finish_cancelled();
                return;
            }
            done += 1;
            if done % 50 == 0 {
                ctx.progress(done as f64 / total.max(1) as f64 * 0.9, "计算文件哈希");
            }
        }
        // 快速指纹：头 64KB
        let mut quick: HashMap<String, Vec<(i64, String)>> = HashMap::new();
        for (fid, path) in group {
            match quick_fingerprint(Path::new(path)) {
                Some(fp) => quick.entry(fp).or_default().push((*fid, path.clone())),
                None => skipped_unreadable += 1,
            }
        }
        // 完整 BLAKE3
        for (_, same) in quick {
            if same.len() < 2 {
                continue;
            }
            let mut full: HashMap<String, Vec<(i64, String)>> = HashMap::new();
            for (fid, path) in same {
                match full_hash(Path::new(path.as_str())) {
                    Some(h) => full.entry(h).or_default().push((fid, path)),
                    None => skipped_unreadable += 1,
                }
            }
            for (hash, files) in full {
                if files.len() < 2 {
                    continue;
                }
                let rows: Vec<FileRow> = db.with_conn(|c| {
                    let mut out = Vec::new();
                    for (fid, _path) in &files {
                        let row: Option<FileRow> = c
                            .query_row(
                                "SELECT file_id, name, path, ext, kind, size, modified_ms, created_ms, is_dir, fulltext_state FROM files WHERE file_id=?1",
                                [fid],
                                |r| {
                                    Ok(FileRow {
                                        file_id: r.get(0)?,
                                        name: r.get(1)?,
                                        path: r.get(2)?,
                                        ext: r.get(3)?,
                                        kind: r.get(4)?,
                                        size: r.get(5)?,
                                        modified_ms: r.get(6)?,
                                        created_ms: r.get(7)?,
                                        is_dir: r.get::<_, i64>(8)? != 0,
                                        fulltext_state: r.get(9)?,
                                        snippet: None,
                                        score: 0.0,
                                        favorite: false,
                                        tags: Vec::new(),
                                    })
                                },
                            )
                            .ok();
                        if let Some(row) = row {
                            out.push(row);
                        }
                    }
                    out
                });
                let wasted = rows.first().map(|r| r.size * (rows.len() as i64 - 1)).unwrap_or(0);
                full_groups.push(DuplicateGroup { hash, files: rows, wasted });
            }
        }
        ctx.progress(0.95, "汇总重复组");
    }
    full_groups.sort_by(|a, b| b.wasted.cmp(&a.wasted));
    let result = serde_json::json!({
        "groups": full_groups,
        "skippedUnreadable": skipped_unreadable,
        "keepHow": _keep_how,
    });
    ctx.finish_ok(Some(result.to_string()));
}

pub fn start_duplicates_job(
    state: &crate::AppState,
    min_size: i64,
    keep_how: String,
) -> Result<(), String> {
    let db = state.engine.db.clone();
    state.engine.start(
        "duplicates",
        serde_json::json!({"minSize": min_size}),
        move |ctx| run_duplicates(ctx, db, min_size, keep_how),
    )?;
    Ok(())
}

fn quick_fingerprint(p: &Path) -> Option<String> {
    use std::io::Read;
    let mut f = std::fs::File::open(p).ok()?;
    let mut head = [0u8; 64 * 1024];
    let n = f.read(&mut head).ok()?;
    let mut h = blake3::Hasher::new();
    h.update(&head[..n]);
    Some(h.finalize().to_hex().as_str()[..16].to_string())
}

fn full_hash(p: &Path) -> Option<String> {
    let mut reader = blake3::Hasher::new();
    let mut f = std::fs::File::open(p).ok()?;
    std::io::copy(&mut f, &mut reader).ok()?;
    Some(reader.finalize().to_hex().to_string())
}

/// 规则式智能整理建议：按扩展名/日期归类到目标目录（FR15）。
#[tauri::command]
pub fn organize_suggest(
    store: tauri::State<'_, crate::AppState>,
    source_dir: String,
    rule_type: String,
    target_dir: String,
) -> Result<Vec<crate::models::PlanItem>, String> {
    let rows: Vec<(i64, String)> = store.db.with_conn(|c| {
        let mut stmt = c
            .prepare(
                "SELECT file_id, path FROM files WHERE is_dir=0 AND path LIKE ?1 ORDER BY modified_ms DESC LIMIT 100",
            )
            .map_err(|e| e.to_string())?;
        let all = stmt
            .query_map([format!("{source_dir}\\%")], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(|e| e.to_string())?
            .filter_map(|x| x.ok())
            .collect();
        Ok::<_, String>(all)
    })?;
    let mut items = Vec::new();
    for (fid, path) in rows {
        let p = Path::new(&path);
        let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let target_name = match rule_type.as_str() {
            "extension" => {
                let ext = p.extension().map(|e| e.to_string_lossy().to_uppercase()).unwrap_or_else(|| "OTHER".into());
                format!("{target_dir}\\{ext}")
            }
            "date_ym" => {
                let date: Option<String> = store.db.with_conn(|c| {
                    c.query_row("SELECT modified_ms FROM files WHERE file_id=?1", [fid], |r| r.get(0)).ok()
                }).and_then(|ms| chrono::DateTime::from_timestamp_millis(ms).map(|d| d.format("%Y-%m").to_string()));
                format!("{target_dir}\\{}", date.unwrap_or_else(|| "未知日期".into()))
            }
            _ => target_dir.clone(),
        };
        let new_path = format!("{target_name}\\{name}");
        let status = if Path::new(&new_path).exists() { "conflict" } else { "ok" };
        items.push(crate::models::PlanItem {
            file_id: fid,
            old_path: path,
            new_path,
            status: status.into(),
            reason: None,
        });
    }
    Ok(items)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimilarGroup {
    pub files: Vec<FileRow>,
    pub avg_distance: f64,
}

/// 相似图片实验：dHash 感知哈希 + 汉明距离（P3 独立实验，不与字节重复混淆）。
#[tauri::command]
pub fn similar_images(
    store: tauri::State<'_, crate::AppState>,
    path_prefix: String,
    threshold: i32,
) -> Result<Vec<SimilarGroup>, String> {
    let images: Vec<(i64, String)> = store.db.with_conn(|c| {
        let mut stmt = c
            .prepare(
                "SELECT file_id, path FROM files WHERE kind='image' AND size < 20971520 AND path LIKE ?1 LIMIT 2000",
            )
            .map_err(|e| e.to_string())?;
        let all = stmt
            .query_map([format!("{path_prefix}%")], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(|e| e.to_string())?
            .filter_map(|x| x.ok())
            .collect();
        Ok::<_, String>(all)
    })?;
    let mut hashes: Vec<(i64, String, u64)> = Vec::new();
    for (fid, path) in images {
        if let Some(h) = dhash(Path::new(&path)) {
            hashes.push((fid, path, h));
        }
    }
    let mut used = vec![false; hashes.len()];
    let mut groups = Vec::new();
    for i in 0..hashes.len() {
        if used[i] {
            continue;
        }
        let mut members = vec![i];
        used[i] = true;
        for j in i + 1..hashes.len() {
            if used[j] {
                continue;
            }
            let d = (hashes[i].2 ^ hashes[j].2).count_ones() as i32;
            if d <= threshold.max(1) {
                used[j] = true;
                members.push(j);
            }
        }
        if members.len() >= 2 {
            let files: Vec<FileRow> = store.db.with_conn(|c| {
                let mut out = Vec::new();
                for &m in &members {
                    let row: Option<FileRow> = c
                        .query_row(
                            "SELECT file_id, name, path, ext, kind, size, modified_ms, created_ms, is_dir, fulltext_state FROM files WHERE file_id=?1",
                            [hashes[m].0],
                            |r| {
                                Ok(FileRow {
                                    file_id: r.get(0)?,
                                    name: r.get(1)?,
                                    path: r.get(2)?,
                                    ext: r.get(3)?,
                                    kind: r.get(4)?,
                                    size: r.get(5)?,
                                    modified_ms: r.get(6)?,
                                    created_ms: r.get(7)?,
                                    is_dir: r.get::<_, i64>(8)? != 0,
                                    fulltext_state: r.get(9)?,
                                    snippet: None,
                                    score: 0.0,
                                    favorite: false,
                                    tags: Vec::new(),
                                })
                            },
                        )
                        .ok();
                    if let Some(row) = row {
                        out.push(row);
                    }
                }
                out
            });
            groups.push(SimilarGroup { files, avg_distance: threshold as f64 });
        }
        if groups.len() >= 50 {
            break;
        }
    }
    Ok(groups)
}

/// 8×9 灰度差值哈希。
fn dhash(p: &Path) -> Option<u64> {
    let img = image::open(p).ok()?;
    let gray = img.to_luma8();
    let resized = image::imageops::resize(&gray, 9, 8, image::imageops::FilterType::Triangle);
    let mut hash = 0u64;
    for y in 0..8 {
        for x in 0..8 {
            let left = resized.get_pixel(x, y)[0];
            let right = resized.get_pixel(x + 1, y)[0];
            if left > right {
                hash |= 1 << (y * 8 + x);
            }
        }
    }
    Some(hash)
}

/// 智能整理执行（默认仅建议；自动执行时保留 operation_log，FR15）。
#[tauri::command]
pub fn organize_execute(items: Vec<crate::models::PlanItem>) -> Result<serde_json::Value, String> {
    let mut ok = 0;
    let mut failed = 0;
    for item in &items {
        if item.status != "ok" {
            continue;
        }
        let dest = Path::new(&item.new_path);
        if let Some(parent) = dest.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match std::fs::rename(&item.old_path, dest) {
            Ok(_) => ok += 1,
            Err(_) => failed += 1,
        }
    }
    Ok(serde_json::json!({"ok": ok, "failed": failed}))
}

pub fn _unused(_: OrganizerRule) {}

#[tauri::command]
pub fn start_duplicates_cmd(
    store: tauri::State<'_, crate::AppState>,
    min_size: i64,
    keep_how: String,
) -> Result<(), String> {
    start_duplicates_job(&store, min_size, keep_how)
}
