use std::path::Path;

use rusqlite::params;

use crate::jobs::JobContext;
use crate::util;

const MAX_CONTENT_FILE: i64 = 50 * 1024 * 1024; // 单文件正文上限 50MB（FR06）

fn supported_ext(ext: &str) -> Option<&'static str> {
    Some(match ext {
        "txt" | "md" | "json" | "xml" | "csv" | "log" | "html" | "htm" => "text",
        "rs" | "py" | "js" | "ts" | "tsx" | "jsx" | "go" | "java" | "c" | "cpp" | "h" | "cs"
        | "css" | "sql" | "sh" | "ps1" | "bat" | "yml" | "yaml" | "toml" | "ini" => "text",
        "docx" => "docx",
        "xlsx" => "xlsx",
        "pptx" => "pptx",
        "pdf" => "pdf",
        _ => return None,
    })
}

/// 全文提取任务：按范围处理 pending 文件，写入 content_text + outbox（FR06）。
pub fn run_fulltext(
    ctx: JobContext,
    db: std::sync::Arc<crate::settings::SettingsStore>,
    scope_only: Option<i64>,
) {
    let files: Result<Vec<(i64, String, String, i64, i64)>, String> = db.with_conn(|c| {
        let sql = "SELECT file_id, path, ext, size, modified_ms FROM files
             WHERE is_dir=0 AND (?1 IS NULL OR scope_id = ?1)
             AND fulltext_state IN ('none','pending','failed') LIMIT 20000";
        let mut stmt = c.prepare(sql).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![scope_only], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })
            .map_err(|e| e.to_string())?;
        let all: Vec<(i64, String, String, i64, i64)> = rows.filter_map(|x| x.ok()).collect();
        Ok::<_, String>(all)
    });
    let files = match files {
        Ok(v) => v,
        Err(e) => {
            ctx.finish_failed(&e);
            return;
        }
    };
    let total = files.len() as f64;
    let mut done = 0f64;
    let mut ok_count = 0u64;
    let mut fail_count = 0u64;
    for (fid, path, ext, size, modified) in files {
        if ctx.cancelled() {
            ctx.finish_cancelled();
            return;
        }
        ctx.wait_if_paused();
        done += 1.0;
        if done % 20.0 == 0.0 {
            ctx.progress(done / total.max(1.0), &format!("提取 {} / {}", done as u64, total as u64));
        }
        let Some(extractor) = supported_ext(&ext) else {
            continue;
        };
        // OneDrive 占位与超限文件只索引元数据（§6.3、FR06）
        let p = Path::new(&path);
        if size > MAX_CONTENT_FILE {
            mark_state(db.clone(), fid, "skipped", "too_large", extractor, modified);
            continue;
        }
        let attrs = std::fs::metadata(p)
            .map(|m| {
                use std::os::windows::fs::MetadataExt;
                m.file_attributes()
            })
            .unwrap_or(0);
        if util::is_cloud_placeholder(attrs) {
            mark_state(db.clone(), fid, "skipped", "cloud_placeholder", extractor, modified);
            continue;
        }
        // 单文件超时隔离：卡住的解析器不会拖死整个任务（FR06）
        let p_t = p.to_path_buf();
        let ext_t = extractor.to_string();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name(format!("extract-{}", ext_t))
            .spawn(move || {
                let _ = tx.send(extract_text(&p_t, &ext_t));
            })
            .ok();
        let result = match rx.recv_timeout(std::time::Duration::from_secs(20)) {
            Ok(r) => r,
            Err(_) => Err("extraction_timeout".to_string()),
        };
        match result {
            Ok(text) => {
                db.with_conn(|c| {
                    // 全部写入共用同一次加锁：mark_state 内部也会加锁，嵌套调用会自死锁
                    c.execute("DELETE FROM content_text WHERE file_id=?1", [fid]).ok();
                    c.execute(
                        "INSERT INTO content_text(file_id, content_text) VALUES (?1, ?2)",
                        params![fid, text],
                    )
                    .ok();
                    c.execute(
                        "INSERT INTO content_state(file_id, extractor, status, error_code, indexed_ms)
                         VALUES (?1,?2,'done',NULL,?3)
                         ON CONFLICT(file_id) DO UPDATE SET
                           extractor=excluded.extractor, status=excluded.status,
                           error_code=NULL, indexed_ms=excluded.indexed_ms",
                        params![fid, extractor, util::now_ms()],
                    )
                    .ok();
                    c.execute(
                        "UPDATE files SET fulltext_state='done' WHERE file_id=?1",
                        [fid],
                    )
                    .ok();
                    c.execute(
                        "INSERT INTO index_outbox(file_id, operation, created_ms) VALUES (?1,'upsert',?2)",
                        params![fid, util::now_ms()],
                    )
                    .ok();
                    Ok::<(), String>(())
                })
                .ok();
                ok_count += 1;
            }
            Err(e) => {
                mark_state(db.clone(), fid, "failed", &e, extractor, modified);
                fail_count += 1;
            }
        }
    }
    ctx.finish_ok(Some(
        serde_json::json!({"extracted": ok_count, "failed": fail_count}).to_string(),
    ));
}

fn mark_state(
    db: std::sync::Arc<crate::settings::SettingsStore>,
    fid: i64,
    status: &str,
    error: &str,
    extractor: &str,
    _modified: i64,
) {
    db.with_conn(|c| {
        c.execute(
            "INSERT INTO content_state(file_id, extractor, status, error_code, indexed_ms)
             VALUES (?1,?2,?3,?4,?5)
             ON CONFLICT(file_id) DO UPDATE SET
               extractor=excluded.extractor, status=excluded.status,
               error_code=excluded.error_code, indexed_ms=excluded.indexed_ms",
            params![fid, extractor, status, error, util::now_ms()],
        )
        .ok();
        c.execute(
            "UPDATE files SET fulltext_state=?2 WHERE file_id=?1",
            params![fid, if status == "done" { "done" } else if status == "skipped" { "skipped" } else { "failed" }],
        )
        .ok();
        Ok::<(), String>(())
    })
    .ok();
}

pub fn start_fulltext_job(
    state: &crate::AppState,
    scope_id: Option<i64>,
) -> Result<(), String> {
    let db = state.engine.db.clone();
    state
        .engine
        .start("fulltext", serde_json::json!({"scopeId": scope_id}), move |ctx| {
            run_fulltext(ctx, db, scope_id)
        })?;
    Ok(())
}

pub fn extract_text(p: &Path, extractor: &str) -> Result<String, String> {
    match extractor {
        "text" => read_text_lossy(p),
        "docx" => extract_docx(p),
        "xlsx" => extract_xlsx(p),
        "pptx" => extract_pptx(p),
        "pdf" => extract_pdf(p),
        _ => Err("unsupported".into()),
    }
}

fn read_text_lossy(p: &Path) -> Result<String, String> {
    let bytes = std::fs::read(p).map_err(|e| e.to_string())?;
    // 常见中文场景：UTF-8 失败时回退 GBK（encoding_rs）
    match String::from_utf8(bytes.clone()) {
        Ok(s) => Ok(s),
        Err(_) => {
            let (decoded, _, had_errors) = encoding_rs::GBK.decode(&bytes);
            if had_errors {
                Ok(decoded.to_string())
            } else {
                Ok(decoded.to_string())
            }
        }
    }
}

fn strip_xml_tags(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len() / 2);
    let mut in_tag = false;
    let mut chars = xml.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => {
                if c == '&' {
                    // 基础实体
                    let mut entity = String::new();
                    for e in chars.by_ref() {
                        if e == ';' {
                            break;
                        }
                        entity.push(e);
                        if entity.len() > 10 {
                            break;
                        }
                    }
                    match entity.as_str() {
                        "amp" => out.push('&'),
                        "lt" => out.push('<'),
                        "gt" => out.push('>'),
                        "quot" => out.push('"'),
                        "apos" => out.push('\''),
                        _ => {}
                    }
                } else {
                    out.push(c);
                }
            }
            _ => {}
        }
    }
    out
}

fn zip_read_entry(archive_path: &Path, names: &[&str]) -> Option<String> {
    let f = std::fs::File::open(archive_path).ok()?;
    let mut zip = zip::ZipArchive::new(f).ok()?;
    for name in names {
        if let Ok(mut entry) = zip.by_name(name) {
            let mut buf = String::new();
            std::io::Read::read_to_string(&mut entry, &mut buf).ok()?;
            return Some(buf);
        }
    }
    None
}

fn extract_docx(p: &Path) -> Result<String, String> {
    let xml = zip_read_entry(p, &["word/document.xml"]).ok_or("无法读取 docx 内容")?;
    Ok(strip_xml_tags(&xml))
}

fn extract_xlsx(p: &Path) -> Result<String, String> {
    let shared = zip_read_entry(p, &["xl/sharedStrings.xml"]).ok_or("无法读取 xlsx 内容")?;
    Ok(strip_xml_tags(&shared))
}

fn extract_pptx(p: &Path) -> Result<String, String> {
    let f = std::fs::File::open(p).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(f).map_err(|e| e.to_string())?;
    let mut all = String::new();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().to_string();
        if name.starts_with("ppt/slides/slide") && name.ends_with(".xml") {
            let mut buf = String::new();
            std::io::Read::read_to_string(&mut entry, &mut buf).ok();
            all.push_str(&strip_xml_tags(&buf));
            all.push(' ');
        }
    }
    if all.is_empty() {
        Err("无幻灯片文本".into())
    } else {
        Ok(all)
    }
}

fn extract_pdf(p: &Path) -> Result<String, String> {
    let bytes = std::fs::read(p).map_err(|e| e.to_string())?;
    let text = pdf_extract::extract_text_from_mem(&bytes).map_err(|e| e.to_string())?;
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_tags_basic() {
        assert_eq!(strip_xml_tags("<w:p>你好</w:p><w:t>World</w:t>"), "你好World");
        assert_eq!(strip_xml_tags("a &amp; b"), "a & b");
    }
}

#[tauri::command]
pub fn cmd_fulltext_start(
    store: tauri::State<'_, std::sync::Arc<crate::AppState>>,
    scope_id: Option<i64>,
) -> Result<(), String> {
    start_fulltext_job(&store, scope_id)
}
