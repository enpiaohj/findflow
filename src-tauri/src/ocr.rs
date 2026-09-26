use std::path::Path;

use rusqlite::params;

use crate::jobs::JobContext;
use crate::util;

const MAX_OCR_IMAGE: i64 = 20 * 1024 * 1024;
// OCR 资源限制：每天处理量上限（FR17）
const DAILY_OCR_LIMIT: u64 = 5_000;

/// OCR 任务：对范围内图片提取文字（Windows.Media.Ocr），文本标记来源与置信度。
pub fn run_ocr(
    ctx: JobContext,
    db: std::sync::Arc<crate::settings::DbHandle>,
    scope_id: Option<i64>,
) {
    let files: Vec<(i64, String, i64)> = match db.with_conn(|c| {
        let sql = "SELECT file_id, path, size FROM files WHERE is_dir=0 AND (?1 IS NULL OR scope_id=?1)
             AND kind='image' AND ext IN ('png','jpg','jpeg','bmp') AND ocr_state IN ('none','failed')
             AND size < ?2 LIMIT 5000";
        let mut stmt = c.prepare(sql).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![scope_id, MAX_OCR_IMAGE], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .map_err(|e| e.to_string())?;
        let all: Vec<(i64, String, i64)> = rows.filter_map(|x| x.ok()).collect();
        Ok::<_, String>(all)
    }) {
        Ok(v) => v,
        Err(e) => {
            ctx.finish_failed(&e);
            return;
        }
    };
    let total = files.len() as f64;
    let mut done = 0f64;
    let mut ok = 0u64;
    let mut fail = 0u64;

    let engine = match ocr_engine() {
        Ok(e) => e,
        Err(e) => {
            ctx.finish_failed(&e);
            return;
        }
    };

    for (fid, path, _size) in files {
        if ctx.cancelled() {
            ctx.finish_cancelled();
            return;
        }
        ctx.wait_if_paused();
        let processed: u64 = db.with_conn(|c| {
            c.query_row(
                "SELECT value_json FROM settings WHERE key='ocr.processed_today'",
                [],
                |r| r.get::<_, String>(0),
            )
            .ok()
            .and_then(|s| serde_json::from_str::<String>(&s).ok())
            .and_then(|v| v.parse().ok())
            .unwrap_or(0)
        });
        if processed >= DAILY_OCR_LIMIT {
            ctx.finish_ok(Some(
                serde_json::json!({"done": ok, "failed": fail, "pausedByDailyLimit": true}).to_string(),
            ));
            return;
        }
        done += 1.0;
        if done % 10.0 == 0.0 {
            ctx.progress(done / total.max(1.0), &format!("OCR {} / {}", done as u64, total as u64));
        }
        match ocr_image(&engine, Path::new(&path)) {
            Ok(text) => {
                ok += 1;
                db.with_conn(|c| {
                    if text.trim().is_empty() {
                        c.execute("UPDATE files SET ocr_state='done' WHERE file_id=?1", [fid]).ok();
                    } else {
                        c.execute("DELETE FROM ocr_text WHERE file_id=?1", [fid]).ok();
                        c.execute(
                            "INSERT INTO ocr_text(file_id, ocr_text) VALUES (?1, ?2)",
                            params![fid, text],
                        )
                        .ok();
                        c.execute("UPDATE files SET ocr_state='done' WHERE file_id=?1", [fid]).ok();
                        c.execute(
                            "INSERT INTO index_outbox(file_id, operation, created_ms) VALUES (?1,'upsert',?2)",
                            params![fid, util::now_ms()],
                        )
                        .ok();
                    }
                    Ok::<(), String>(())
                })
                .ok();
            }
            Err(_) => {
                fail += 1;
                db.with_conn(|c| {
                    c.execute("UPDATE files SET ocr_state='failed' WHERE file_id=?1", [fid]).ok();
                    Ok::<(), String>(())
                })
                .ok();
            }
        }
        bump_daily_counter(&db, processed + 1);
    }
    ctx.finish_ok(Some(serde_json::json!({"done": ok, "failed": fail}).to_string()));
}

fn bump_daily_counter(db: &std::sync::Arc<crate::settings::DbHandle>, value: u64) {
    db.with_conn(|c| {
            c.execute(
                "INSERT INTO settings(key, value_json, updated_at) VALUES ('ocr.processed_today', ?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value_json=?1, updated_at=?2",
                params![value.to_string(), util::now_rfc3339()],
            )
        })
        .ok();
}

fn ocr_engine() -> Result<windows::Media::Ocr::OcrEngine, String> {
    use windows::Globalization::ApplicationLanguages;
    use windows::Media::Ocr::OcrEngine;
    let _ = ApplicationLanguages;
    OcrEngine::TryCreateFromUserProfileLanguages().map_err(|_| "无可用的 OCR 语言包，请在系统设置中添加语言".into())
}

fn ocr_image(
    engine: &windows::Media::Ocr::OcrEngine,
    path: &Path,
) -> Result<String, String> {
    use windows::Foundation::Uri;
    use windows::Graphics::Imaging::BitmapDecoder;
    use windows::Storage::StorageFile;
    use windows::core::HSTRING;

    let path_h = HSTRING::from(path.to_string_lossy().as_ref());
    // StorageFile 需要文件协议 URI
    let uri_str = HSTRING::from(format!(
        "file:///{}",
        path.to_string_lossy().replace('\\', "/")
    ));
    let uri = Uri::CreateUri(&uri_str).map_err(|e| e.to_string())?;
    let file = StorageFile::GetFileFromPathAsync(&path_h)
        .and_then(|op| op.get())
        .or_else(|_| StorageFile::GetFileFromApplicationUriAsync(&uri).and_then(|op| op.get()))
        .map_err(|e| format!("无法打开图片：{e}"))?;
    let stream = file
        .OpenAsync(windows::Storage::FileAccessMode::Read)
        .and_then(|op| op.get())
        .map_err(|e| e.to_string())?;
    let decoder = BitmapDecoder::CreateAsync(&stream)
        .and_then(|op| op.get())
        .map_err(|e| e.to_string())?;
    let bitmap = decoder
        .GetSoftwareBitmapAsync()
        .and_then(|op| op.get())
        .map_err(|e| e.to_string())?;
    let result = engine
        .RecognizeAsync(&bitmap)
        .and_then(|op| op.get())
        .map_err(|e| e.to_string())?;
    let text = result
        .Text()
        .map_err(|e| e.to_string())?
        .to_string_lossy()
        .to_string();
    Ok(text)
}

pub fn start_ocr_job(
    state: &crate::AppState,
    scope_id: Option<i64>,
) -> Result<(), String> {
    let db = state.bg_db.clone();
    state
        .engine
        .start("ocr", serde_json::json!({"scopeId": scope_id}), move |ctx| {
            run_ocr(ctx, db, scope_id)
        })?;
    Ok(())
}

#[tauri::command]
pub async fn cmd_ocr_start(
    store: tauri::State<'_, std::sync::Arc<crate::AppState>>,
    scope_id: Option<i64>,
) -> Result<(), String> {
    start_ocr_job(&store, scope_id)
}
