use std::collections::HashMap;
use std::path::Path;

use rusqlite::params;
use serde::Serialize;

use crate::jobs::JobContext;
use crate::models::{DuplicateGroup, FileRow, OrganizerRule};

/// 大文件分析（FR13）：按范围取最大的 N 个文件，删除前会重新读取属性。
#[tauri::command]
pub async fn large_files(
    store: tauri::State<'_, std::sync::Arc<crate::AppState>>,
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
    db: std::sync::Arc<crate::settings::DbHandle>,
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
    full_groups.sort_by_key(|g| std::cmp::Reverse(g.wasted));
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
    let db = state.bg_db.clone();
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

/// 按来源目录与规则生成整理计划；`organize_suggest` 与 `organize_execute` 共用，
/// 保证「预演所见 = 实际执行」（FR15）。
fn build_plan(
    db: &crate::settings::SettingsStore,
    source_dir: &str,
    rule_type: &str,
    target_dir: &str,
) -> Result<Vec<crate::models::PlanItem>, String> {
    let rows: Vec<(i64, String, i64)> = db.with_conn(|c| {
        let mut stmt = c
            .prepare(
                "SELECT file_id, path, modified_ms FROM files WHERE is_dir=0 AND path LIKE ?1 ORDER BY modified_ms DESC LIMIT 100",
            )
            .map_err(|e| e.to_string())?;
        let all = stmt
            .query_map([format!("{source_dir}\\%")], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .map_err(|e| e.to_string())?
            .filter_map(|x| x.ok())
            .collect();
        Ok::<_, String>(all)
    })?;
    let mut items = Vec::new();
    for (fid, path, modified_ms) in rows {
        let p = Path::new(&path);
        let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let sub = match rule_type {
            "extension" => p
                .extension()
                .map(|e| e.to_string_lossy().to_uppercase())
                .unwrap_or_else(|| "OTHER".into()),
            "date_ym" => chrono::DateTime::from_timestamp_millis(modified_ms)
                .map(|d| d.format("%Y-%m").to_string())
                .unwrap_or_else(|| "未知日期".into()),
            _ => String::new(),
        };
        let target_name = if sub.is_empty() {
            target_dir.to_string()
        } else {
            format!("{target_dir}\\{sub}")
        };
        let new_path = format!("{target_name}\\{name}");
        // 底线 3：源与目标都不得落在受保护目录。
        let (status, reason) = if crate::scope::is_protected(&crate::util::normalize_path(p))
            || crate::scope::is_protected(&new_path)
        {
            ("protected", Some("受保护目录，不予整理".to_string()))
        } else if !p.exists() {
            ("missing", Some("源文件不存在".to_string()))
        } else if Path::new(&new_path).exists() {
            ("conflict", Some("目标已存在同名文件".to_string()))
        } else {
            ("ok", None)
        };
        items.push(crate::models::PlanItem {
            file_id: fid,
            old_path: path,
            new_path,
            status: status.into(),
            reason,
        });
    }
    Ok(items)
}

/// 规则式智能整理建议：按扩展名/日期归类到目标目录（FR15）。
#[tauri::command]
pub async fn organize_suggest(
    store: tauri::State<'_, std::sync::Arc<crate::AppState>>,
    source_dir: String,
    rule_type: String,
    target_dir: String,
) -> Result<Vec<crate::models::PlanItem>, String> {
    build_plan(&store.db, &source_dir, &rule_type, &target_dir)
}

/// 整理执行前的确认令牌（绑定来源目录 / 规则 / 目标目录，60 秒有效、一次性）。
#[tauri::command]
pub async fn organize_issue_token(
    source_dir: String,
    rule_type: String,
    target_dir: String,
) -> Result<String, String> {
    crate::actions::issue_token(&crate::actions::fingerprint(&[
        &source_dir,
        &rule_type,
        &target_dir,
    ]))
}

/// 智能整理执行（FR15）：后端按参数重新生成计划，不信任前端传入的路径。
/// 逐项返回结果并写 operation_log，保证可追溯（底线 2）。
#[tauri::command]
pub async fn organize_execute(
    store: tauri::State<'_, std::sync::Arc<crate::AppState>>,
    source_dir: String,
    rule_type: String,
    target_dir: String,
    token: String,
) -> Result<Vec<crate::models::PlanItem>, String> {
    // 令牌绑定三个参数：换了目录或规则即失效，且消费后不可重放。
    let fp = crate::actions::fingerprint(&[&source_dir, &rule_type, &target_dir]);
    if !crate::actions::consume_token(&token, &fp) {
        return Err("整理确认令牌无效或已过期，请重新确认".into());
    }
    let mut items = build_plan(&store.db, &source_dir, &rule_type, &target_dir)?;
    let op_id = crate::util::now_ms();
    let mut ok = 0usize;
    let mut failed = 0usize;
    for it in items.iter_mut() {
        if it.status != "ok" {
            continue;
        }
        let dest = Path::new(&it.new_path);
        if let Some(parent) = dest.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                it.status = "failed".into();
                it.reason = Some(format!("创建目录失败：{e}"));
                failed += 1;
                continue;
            }
        }
        match std::fs::rename(&it.old_path, dest) {
            Ok(_) => {
                store.db.with_conn(|c| {
                    let name = Path::new(&it.new_path)
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    c.execute(
                        "UPDATE files SET path=?2, name=?3, updated_ms=?4 WHERE file_id=?1",
                        params![it.file_id, it.new_path, name, crate::util::now_ms()],
                    )
                    .ok();
                    c.execute(
                        "INSERT INTO index_outbox(file_id, operation, created_ms) VALUES (?1,'upsert',?2)",
                        params![it.file_id, crate::util::now_ms()],
                    )
                    .ok();
                    Ok::<(), String>(())
                })
                .ok();
                it.status = "ok".into();
                it.reason = None;
                ok += 1;
            }
            Err(e) => {
                it.status = "failed".into();
                it.reason = Some(e.to_string());
                failed += 1;
            }
        }
    }
    store
        .db
        .with_conn(|c| {
            c.execute(
                "INSERT INTO operation_log(operation_id, kind, request_json, result_json, reversible_until, created_at) VALUES (?1,'organize',?2,?3,?4,?5)",
                params![
                    op_id,
                    serde_json::json!({
                        "sourceDir": source_dir,
                        "ruleType": rule_type,
                        "targetDir": target_dir,
                    })
                    .to_string(),
                    serde_json::json!({"ok": ok, "failed": failed, "total": items.len()}).to_string(),
                    (chrono::Local::now() + chrono::Duration::hours(24)).to_rfc3339(),
                    crate::util::now_rfc3339()
                ],
            )
            .ok();
            Ok::<(), String>(())
        })
        .ok();
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
pub async fn similar_images(
    store: tauri::State<'_, std::sync::Arc<crate::AppState>>,
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

pub fn _unused(_: OrganizerRule) {}

#[tauri::command]
pub async fn start_duplicates_cmd(
    store: tauri::State<'_, std::sync::Arc<crate::AppState>>,
    min_size: i64,
    keep_how: String,
) -> Result<(), String> {
    start_duplicates_job(&store, min_size, keep_how)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> crate::settings::SettingsStore {
        crate::settings::SettingsStore::new(crate::db::open_in_memory().unwrap())
    }

    /// 临时目录放在 target/ 下：系统 temp 位于 AppData，会被 is_protected 命中。
    fn tmpdir() -> tempfile::TempDir {
        let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("testtmp");
        std::fs::create_dir_all(&base).unwrap();
        tempfile::Builder::new().tempdir_in(&base).unwrap()
    }

    fn seed(s: &crate::settings::SettingsStore, fid: i64, path: &str, modified_ms: i64) {
        let name = Path::new(path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        s.with_conn(|c| {
            c.execute(
                "INSERT OR IGNORE INTO scopes(scope_id, path, added_at) VALUES (1,'D:\\s','now')",
                [],
            )
            .unwrap();
            c.execute(
                "INSERT INTO files(file_id, scope_id, name, path, ext, kind, size, is_dir, modified_ms, updated_ms)
                 VALUES (?1, 1, ?2, ?3, 'txt', 'doc', 1, 0, ?4, 1)",
                rusqlite::params![fid, name, path, modified_ms],
            )
            .unwrap();
            Ok::<(), String>(())
        })
        .unwrap();
    }

    #[test]
    fn build_plan_marks_protected_source() {
        let s = db();
        seed(&s, 1, r"C:\Windows\system.ini", 1_700_000_000_000);
        let items = build_plan(&s, r"C:\Windows", "extension", r"D:\整理").unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].status, "protected");
        assert!(items[0].reason.is_some());
    }

    #[test]
    fn build_plan_marks_protected_target() {
        let s = db();
        seed(&s, 1, r"D:\s\a.txt", 1_700_000_000_000);
        let items = build_plan(&s, r"D:\s", "extension", r"C:\Program Files").unwrap();
        assert_eq!(items[0].status, "protected");
    }

    #[test]
    fn build_plan_marks_missing_source() {
        let s = db();
        seed(&s, 1, r"D:\s\不存在.txt", 1_700_000_000_000);
        let items = build_plan(&s, r"D:\s", "extension", r"D:\整理").unwrap();
        assert_eq!(items[0].status, "missing");
    }

    #[test]
    fn build_plan_ok_and_conflict_with_real_files() {
        let dir = tmpdir();
        let src = dir.path().join("src");
        let dst = dir.path().join("dst");
        std::fs::create_dir_all(src.join("sub")).unwrap();
        std::fs::create_dir_all(dst.join("TXT")).unwrap();
        let src_s = crate::util::normalize_path(&src);
        let dst_s = crate::util::normalize_path(&dst);
        std::fs::write(src.join("a.txt"), b"a").unwrap();
        std::fs::write(src.join("b.txt"), b"b").unwrap();
        // b.txt 的目标位置已有同名文件 → conflict
        std::fs::write(dst.join("TXT").join("b.txt"), b"x").unwrap();

        let s = db();
        seed(&s, 1, &format!("{src_s}\\a.txt"), 1_700_000_000_000);
        seed(&s, 2, &format!("{src_s}\\b.txt"), 1_700_000_000_000);

        let items = build_plan(&s, &src_s, "extension", &dst_s).unwrap();
        let find = |n: &str| items.iter().find(|i| i.old_path.ends_with(n)).unwrap();
        assert_eq!(find("a.txt").status, "ok");
        assert_eq!(find("b.txt").status, "conflict");
        assert_eq!(find("a.txt").new_path, format!("{dst_s}\\TXT\\a.txt"));
    }

    #[test]
    fn build_plan_date_ym_groups_by_month() {
        let dir = tmpdir();
        let src = dir.path().join("src");
        std::fs::create_dir_all(&src).unwrap();
        let src_s = crate::util::normalize_path(&src);
        let dst_s = crate::util::normalize_path(&dir.path().join("dst"));
        std::fs::write(src.join("a.txt"), b"a").unwrap();
        let s = db();
        // 1_700_000_000_000 ms = 2023-11-14 UTC
        seed(&s, 1, &format!("{src_s}\\a.txt"), 1_700_000_000_000);
        let items = build_plan(&s, &src_s, "date_ym", &dst_s).unwrap();
        assert_eq!(items[0].status, "ok");
        assert!(
            items[0].new_path.ends_with("2023-11\\a.txt"),
            "实际：{}",
            items[0].new_path
        );
    }

    #[test]
    fn build_plan_empty_when_source_not_indexed() {
        let s = db();
        seed(&s, 1, r"D:\s\a.txt", 1);
        let items = build_plan(&s, r"D:\other", "extension", r"D:\整理").unwrap();
        assert!(items.is_empty(), "来源目录不在索引中则无计划项");
    }
}
