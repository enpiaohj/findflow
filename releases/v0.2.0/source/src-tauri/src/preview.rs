use std::path::Path;

use base64::Engine;
use rusqlite::params;
use serde::Serialize;

/// 预览载荷（FR08）：轻量预览层使用，不支持时给出降级信息。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewPayload {
    pub kind: String,
    pub path: String,
    pub name: String,
    pub size: i64,
    pub modified_ms: i64,
    /// image: base64 PNG；text: 文本内容；thumbnail: base64 PNG（Shell 缩略图）；none: 不支持
    pub data: Option<String>,
    pub reason: Option<String>,
}

const PREVIEW_TEXT_MAX: i64 = 2 * 1024 * 1024;
const PREVIEW_IMAGE_MAX: i64 = 30 * 1024 * 1024;

#[tauri::command]
pub fn preview_get(path: String) -> Result<PreviewPayload, String> {
    let p = Path::new(&path);
    let meta = std::fs::metadata(p).map_err(|_| "文件不存在或不可访问".to_string())?;
    let name = p
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let ext = p
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let modified_ms = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let base = |kind: &str, data: Option<String>, reason: Option<String>| PreviewPayload {
        kind: kind.into(),
        path: path.clone(),
        name: name.clone(),
        size: meta.len() as i64,
        modified_ms,
        data,
        reason,
    };

    // 图片：解码重编码为预览 PNG
    if matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "bmp" | "gif" | "webp") {
        if meta.len() as i64 > PREVIEW_IMAGE_MAX {
            return Ok(base("image", None, Some("图片过大，仅显示属性".into())));
        }
        if let Ok(img) = image::open(p) {
            let resized = if img.width().max(img.height()) > 1600 {
                img.resize(1600, 1600, image::imageops::FilterType::Triangle)
            } else {
                img
            };
            let mut buf = std::io::Cursor::new(Vec::new());
            if resized
                .write_to(&mut buf, image::ImageFormat::Png)
                .is_ok()
            {
                return Ok(base(
                    "image",
                    Some(base64::engine::general_purpose::STANDARD.encode(buf.get_ref())),
                    None,
                ));
            }
        }
        return Ok(base("image", None, Some("图片解码失败".into())));
    }

    // 文本类
    if matches!(
        ext.as_str(),
        "txt" | "md" | "json" | "xml" | "csv" | "log" | "rs" | "py" | "js" | "ts" | "tsx"
            | "jsx" | "go" | "java" | "c" | "cpp" | "h" | "cs" | "css" | "sql" | "sh"
            | "ps1" | "bat" | "yml" | "yaml" | "toml" | "ini" | "html" | "htm"
    ) {
        if meta.len() as i64 > PREVIEW_TEXT_MAX {
            return Ok(base("text", None, Some("文本过大，仅显示属性".into())));
        }
        let content = crate::extract::extract_text(p, "text")?;
        return Ok(base("text", Some(content), None));
    }

    // Office / PDF：Shell 缩略图 + 属性，点击后由默认应用打开（FR08 首期策略）
    if matches!(ext.as_str(), "docx" | "xlsx" | "pptx" | "doc" | "xls" | "ppt" | "pdf" | "vsdx") {
        let thumb = shell_thumbnail(p, 800).ok();
        let has_thumb = thumb.is_some();
        let reason = if has_thumb {
            None
        } else {
            Some("该类型显示属性与摘要，点击打开查看完整内容".to_string())
        };
        return Ok(base(
            if has_thumb { "thumbnail" } else { "none" },
            thumb,
            reason,
        ));
    }

    // 音视频与其他：仅属性
    Ok(base("none", None, None))
}

/// IShellItemImageFactory 缩略图（不加载第三方 Preview Handler，隔离性最好）。
fn shell_thumbnail(p: &Path, size: i32) -> Result<String, String> {
    use windows::Win32::UI::Shell::{SHCreateItemFromParsingName, IShellItemImageFactory};
    use windows::Win32::Graphics::Gdi::{
        GetDIBits, GetObjectW, BITMAP, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS,
    };
    use windows::core::HSTRING;

    let _path_h = HSTRING::from(p.to_string_lossy().as_ref());
    let item: IShellItemImageFactory = unsafe {
        SHCreateItemFromParsingName(&_path_h, None)
    }
    .map_err(|e| format!("Shell 打开失败：{e}"))?;
    let size_struct = windows::Win32::Foundation::SIZE {
        cx: size,
        cy: size,
    };
    let hbitmap = unsafe {
        item.GetImage(size_struct, windows::Win32::UI::Shell::SIIGBF_THUMBNAILONLY)
    }
    .or_else(|_| unsafe { item.GetImage(size_struct, windows::Win32::UI::Shell::SIIGBF_BIGGERSIZEOK) })
    .map_err(|e| format!("缩略图生成失败：{e}"))?;

    // HBITMAP → BGRA → PNG
    unsafe {
        let mut bmp = BITMAP::default();
        let got = GetObjectW(
            windows::Win32::Graphics::Gdi::HGDIOBJ(hbitmap.0),
            std::mem::size_of::<BITMAP>() as i32,
            Some(&mut bmp as *mut _ as _),
        );
        if got == 0 {
            return Err("读取位图信息失败".into());
        }
        let (w, h) = (bmp.bmWidth, bmp.bmHeight);
        let mut bmi = BITMAPINFO::default();
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = w;
        bmi.bmiHeader.biHeight = -h; // 自顶向下
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = 0; // BI_RGB
        let buf_size = (w * h * 4) as usize;
        let mut buf = vec![0u8; buf_size];
        let hdc = windows::Win32::Graphics::Gdi::GetDC(None);
        let copied = GetDIBits(
            hdc,
            hbitmap,
            0,
            h as u32,
            Some(buf.as_mut_ptr() as _),
            &mut bmi,
            DIB_RGB_COLORS,
        );
        let _ = windows::Win32::Graphics::Gdi::ReleaseDC(None, hdc);
        if copied == 0 {
            return Err("读取位图数据失败".into());
        }
        let mut img = image::RgbaImage::new(w as u32, h as u32);
        for y in 0..h as u32 {
            for x in 0..w as u32 {
                let i = ((y * w as u32 + x) * 4) as usize;
                img.put_pixel(
                    x,
                    y,
                    image::Rgba([buf[i + 2], buf[i + 1], buf[i], 255]),
                );
            }
        }
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png)
            .map_err(|e| e.to_string())?;
        Ok(base64::engine::general_purpose::STANDARD.encode(out.get_ref()))
    }
}

/// 收藏与标签（FR09）。
#[tauri::command]
pub fn favorites_list(store: tauri::State<'_, std::sync::Arc<crate::AppState>>) -> Result<Vec<crate::models::FileRow>, String> {
    store.db.with_conn(|c| {
        let mut stmt = c
            .prepare(
                "SELECT f.file_id, f.name, f.path, f.ext, f.kind, f.size, f.modified_ms, f.created_ms, f.is_dir, f.fulltext_state
                 FROM favorites fav JOIN files f ON f.file_id = fav.file_id
                 ORDER BY fav.created_at DESC LIMIT 500",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], file_row_map)
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    })
}

pub(crate) fn file_row_map(r: &rusqlite::Row) -> rusqlite::Result<FileRow> {
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
}

use crate::models::FileRow;

#[tauri::command]
pub fn favorite_toggle(
    store: tauri::State<'_, std::sync::Arc<crate::AppState>>,
    file_id: i64,
) -> Result<bool, String> {
    store.db.with_conn(|c| {
        let exists: bool = c
            .query_row(
                "SELECT 1 FROM favorites WHERE file_id=?1",
                [file_id],
                |_| Ok(true),
            )
            .unwrap_or(false);
        if exists {
            c.execute("DELETE FROM favorites WHERE file_id=?1", [file_id])
                .map_err(|e| e.to_string())?;
            Ok(false)
        } else {
            let fallback: String = c
                .query_row("SELECT path FROM files WHERE file_id=?1", [file_id], |r| r.get(0))
                .map_err(|e| e.to_string())?;
            c.execute(
                "INSERT INTO favorites(file_id, fallback_path, created_at) VALUES (?1,?2,?3)",
                params![file_id, fallback, crate::util::now_rfc3339()],
            )
            .map_err(|e| e.to_string())?;
            Ok(true)
        }
    })
}

#[tauri::command]
pub fn tags_list(store: tauri::State<'_, std::sync::Arc<crate::AppState>>) -> Result<Vec<serde_json::Value>, String> {
    store.db.with_conn(|c| {
        let mut stmt = c
            .prepare("SELECT tag_id, name, color FROM tags ORDER BY name")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                Ok(serde_json::json!({
                    "tagId": r.get::<_, i64>(0)?,
                    "name": r.get::<_, String>(1)?,
                    "color": r.get::<_, String>(2)?,
                }))
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn tag_create(store: tauri::State<'_, std::sync::Arc<crate::AppState>>, name: String, color: String) -> Result<i64, String> {
    store.db.with_conn(|c| {
        c.execute(
            "INSERT INTO tags(name, color) VALUES (?1, ?2)",
            params![name, color],
        )
        .map_err(|e| e.to_string())?;
        Ok(c.last_insert_rowid())
    })
}

#[tauri::command]
pub fn tag_delete(store: tauri::State<'_, std::sync::Arc<crate::AppState>>, tag_id: i64) -> Result<(), String> {
    store
        .db
        .with_conn(|c| c.execute("DELETE FROM tags WHERE tag_id=?1", [tag_id]))
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn file_tags_set(
    store: tauri::State<'_, std::sync::Arc<crate::AppState>>,
    file_id: i64,
    tag_ids: Vec<i64>,
) -> Result<(), String> {
    store.db.with_conn(|c| {
        c.execute("DELETE FROM file_tags WHERE file_id=?1", [file_id])
            .map_err(|e| e.to_string())?;
        for t in tag_ids {
            c.execute(
                "INSERT OR IGNORE INTO file_tags(file_id, tag_id) VALUES (?1, ?2)",
                params![file_id, t],
            )
            .map_err(|e| e.to_string())?;
        }
        Ok(())
    })
}

#[tauri::command]
pub fn recents_list(store: tauri::State<'_, std::sync::Arc<crate::AppState>>) -> Result<Vec<serde_json::Value>, String> {
    store.db.with_conn(|c| {
        let mut stmt = c
            .prepare(
                "SELECT path, action, happened_at FROM recent_actions ORDER BY id DESC LIMIT 100",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                Ok(serde_json::json!({
                    "path": r.get::<_, String>(0)?,
                    "action": r.get::<_, String>(1)?,
                    "at": r.get::<_, String>(2)?,
                }))
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    })
}

/// 按路径直接取行（行内动作可能作用于索引外文件）。
#[tauri::command]
pub fn file_by_path(
    store: tauri::State<'_, std::sync::Arc<crate::AppState>>,
    path: String,
) -> Result<Option<crate::models::FileRow>, String> {
    store.db.with_conn(|c| {
        let r = c.query_row(
            "SELECT file_id, name, path, ext, kind, size, modified_ms, created_ms, is_dir, fulltext_state FROM files WHERE path=?1",
            [&path],
            file_row_map,
        );
        Ok(r.ok())
    })
}
