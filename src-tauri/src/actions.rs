use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use rusqlite::params;
use tauri::Manager;
use windows::core::{HSTRING, PCWSTR};
use windows::Win32::UI::Shell::{ShellExecuteExW, SHELLEXECUTEINFOW, SEE_MASK_INVOKEIDLIST};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOW;

use crate::util;

#[derive(Debug, Clone, Copy)]
pub enum Conflict {
    Skip,
    Overwrite,
    KeepBoth,
    OnlyNewer,
}

pub enum MoveError {
    Skipped,
    Failed(String),
}

/// 移动或复制单个文件/目录；跨卷移动 = 复制 + 校验（大小）+ 删除源（FR11）。
pub fn move_or_copy(
    src: &Path,
    dest: &Path,
    strategy: Conflict,
    delete_source: bool,
) -> Result<(), MoveError> {
    if !src.exists() {
        return Err(MoveError::Failed("源不存在".into()));
    }
    if dest.exists() {
        match strategy {
            Conflict::Skip => return Err(MoveError::Skipped),
            Conflict::KeepBoth => {
                let alt = alternate_name(dest);
                if delete_source {
                    std::fs::rename(src, &alt).map_err(|e| MoveError::Failed(e.to_string()))?;
                } else {
                    copy_tree(src, &alt).map_err(|e| MoveError::Failed(e.to_string()))?;
                }
                return Ok(());
            }
            Conflict::OnlyNewer => {
                if let (Ok(s), Ok(d)) = (std::fs::metadata(src), std::fs::metadata(dest)) {
                    if s.modified().ok().unwrap_or(std::time::SystemTime::UNIX_EPOCH)
                        <= d.modified().ok().unwrap_or(std::time::SystemTime::UNIX_EPOCH)
                    {
                        return Err(MoveError::Skipped);
                    }
                }
                let _ = std::fs::remove_file(dest);
            }
            Conflict::Overwrite => {
                if dest.is_dir() {
                    std::fs::remove_dir_all(dest).map_err(|e| MoveError::Failed(e.to_string()))?;
                } else {
                    let _ = std::fs::remove_file(dest);
                }
            }
        }
    }
    if delete_source {
        if std::fs::rename(src, dest).is_ok() {
            return Ok(());
        }
        copy_tree(src, dest).map_err(MoveError::Failed)?;
        if src.is_dir() {
            std::fs::remove_dir_all(src).map_err(|e| MoveError::Failed(e.to_string()))?;
        } else {
            verify_copy(src, dest).map_err(MoveError::Failed)?;
            std::fs::remove_file(src).map_err(|e| MoveError::Failed(e.to_string()))?;
        }
    } else if src.is_dir() {
        copy_tree(src, dest).map_err(MoveError::Failed)?;
    } else {
        std::fs::copy(src, dest).map_err(|e| MoveError::Failed(format!("复制失败：{e}")))?;
        verify_copy(src, dest).map_err(MoveError::Failed)?;
    }
    Ok(())
}

fn alternate_name(dest: &Path) -> PathBuf {
    let stem = dest
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let ext = dest
        .extension()
        .map(|s| format!(".{}", s.to_string_lossy()))
        .unwrap_or_default();
    let parent = dest.parent().unwrap_or(Path::new("."));
    for i in 1..1000u32 {
        let candidate = parent.join(format!("{stem} - 副本{i}{ext}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    parent.join(format!("{stem} - 副本{ext}"))
}

pub fn copy_tree(src: &Path, dest: &Path) -> Result<(), String> {
    if src.is_dir() {
        std::fs::create_dir_all(dest).map_err(|e| e.to_string())?;
        for entry in std::fs::read_dir(src).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let to = dest.join(entry.file_name());
            copy_tree(&entry.path(), &to)?;
        }
        Ok(())
    } else {
        std::fs::copy(src, dest)
            .map(|_| ())
            .map_err(|e| format!("复制失败：{e}"))
    }
}

pub fn verify_copy(src: &Path, dest: &Path) -> Result<(), String> {
    let s = std::fs::metadata(src).map_err(|e| e.to_string())?;
    let d = std::fs::metadata(dest).map_err(|e| e.to_string())?;
    if s.len() != d.len() {
        return Err("校验失败：复制后大小不一致".into());
    }
    Ok(())
}

// ---------- Tauri 命令 ----------

#[tauri::command]
pub fn action_open(app: tauri::AppHandle, path: String) -> Result<String, String> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_path(path.clone(), None::<&str>)
        .map_err(|e| format!("打开失败：{e}"))?;
    record_recent(&app, &path, "open");
    Ok("已打开".into())
}

#[tauri::command]
pub fn action_locate(path: String) -> Result<String, String> {
    std::process::Command::new("explorer")
        .arg(format!("/select,{path}"))
        .spawn()
        .map_err(|e| format!("定位失败：{e}"))?;
    Ok("已定位".into())
}

#[tauri::command]
pub fn action_copy_path(path: String, format: String) -> Result<String, String> {
    let text = match format.as_str() {
        "name" => Path::new(&path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.clone()),
        "parent" => Path::new(&path)
            .parent()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.clone()),
        "markdown" => {
            let name = Path::new(&path)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path.clone());
            format!("[{name}](file:///{})", path.replace('\\', "/"))
        }
        _ => path.clone(),
    };
    set_clipboard_text(&text)?;
    Ok("已复制路径".into())
}

fn set_clipboard_text(text: &str) -> Result<(), String> {
    use clipboard_win::formats;
    clipboard_win::set(formats::Unicode, text).map_err(|e| format!("剪贴板写入失败：{e}"))
}

#[tauri::command]
pub fn action_copy_files(paths: Vec<String>) -> Result<String, String> {
    set_clipboard_files(&paths)?;
    Ok(format!("已复制 {} 个文件", paths.len()))
}

/// CF_HDROP：把文件放进剪贴板供资源管理器粘贴。
fn set_clipboard_files(paths: &[String]) -> Result<(), String> {
    use windows::Win32::System::DataExchange::OpenClipboard;
    use windows::Win32::Foundation::HWND;
    // 双 NUL 结尾的 DROPFILES 结构
    let mut files_w: Vec<u16> = Vec::new();
    for p in paths {
        files_w.extend(p.encode_utf16());
        files_w.push(0);
    }
    files_w.push(0);
    let dropfiles_size = 20usize; // DROPFILES = offset(4) + pt(8) + fNC(2) + fWide(2) + padding
    let total = dropfiles_size + files_w.len() * 2;
    #[repr(C)]
    struct DROPFILES {
        pfiles: u32,
        pt_x: i32,
        pt_y: i32,
        fnc: i32,
        fwide: i32,
    }
    let mut payload = vec![0u8; total];
    let df = DROPFILES {
        pfiles: dropfiles_size as u32,
        pt_x: 0,
        pt_y: 0,
        fnc: 0,
        fwide: 1,
    };
    payload[0..4].copy_from_slice(&df.pfiles.to_le_bytes());
    payload[4..8].copy_from_slice(&df.pt_x.to_le_bytes());
    payload[8..12].copy_from_slice(&df.pt_y.to_le_bytes());
    payload[12..16].copy_from_slice(&df.fnc.to_le_bytes());
    payload[16..20].copy_from_slice(&df.fwide.to_le_bytes());
    for (i, w) in files_w.iter().enumerate() {
        payload[dropfiles_size + i * 2..dropfiles_size + i * 2 + 2].copy_from_slice(&w.to_le_bytes());
    }
    unsafe {
        use windows::Win32::System::DataExchange::{CloseClipboard, SetClipboardData};
        use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
        if OpenClipboard(HWND::default()).is_err() {
            return Err("无法打开剪贴板".into());
        }
        let handle = GlobalAlloc(GMEM_MOVEABLE, total).map_err(|e| format!("内存分配失败：{e}"))?;
        let locked = GlobalLock(handle);
        if locked.is_null() {
            let _ = CloseClipboard();
            return Err("剪贴板内存锁定失败".into());
        }
        std::ptr::copy_nonoverlapping(payload.as_ptr(), locked as *mut u8, total);
        let _ = GlobalUnlock(handle);
        // CF_HDROP = 15
        let ok = SetClipboardData(15u32, windows::Win32::Foundation::HANDLE(handle.0));
        let _ = CloseClipboard();
        if ok.is_err() {
            return Err("剪贴板写入失败".into());
        }
    }
    Ok(())
}

fn item(path: &str, status: &str, reason: Option<String>) -> crate::models::PlanItem {
    crate::models::PlanItem {
        file_id: 0,
        old_path: path.into(),
        new_path: String::new(),
        status: status.into(),
        reason,
    }
}

#[derive(serde::Deserialize)]
pub struct DeleteRequest {
    pub paths: Vec<String>,
    pub token: String,
}

#[tauri::command]
pub fn action_recycle(
    app: tauri::AppHandle,
    request: DeleteRequest,
) -> Result<Vec<crate::models::PlanItem>, String> {
    if !verify_token(&request.token, &request.paths) {
        return Err("删除确认令牌无效或已过期，请重新确认".into());
    }
    let mut results = Vec::new();
    for p in &request.paths {
        let target = Path::new(p);
        if !target.exists() {
            results.push(item(p, "missing", Some("目标不存在".into())));
            continue;
        }
        match trash::delete(target) {
            Ok(_) => {
                results.push(item(p, "ok", None));
                mark_deleted(&app, p);
            }
            Err(e) => results.push(item(p, "failed", Some(e.to_string()))),
        }
    }
    Ok(results)
}

#[tauri::command]
pub fn action_delete_permanent(
    app: tauri::AppHandle,
    request: DeleteRequest,
) -> Result<Vec<crate::models::PlanItem>, String> {
    if !verify_token(&request.token, &request.paths) {
        return Err("删除确认令牌无效或已过期，请重新确认".into());
    }
    let mut results = Vec::new();
    for p in &request.paths {
        let target = Path::new(p);
        if !target.exists() {
            results.push(item(p, "missing", Some("目标不存在".into())));
            continue;
        }
        let r = if target.is_dir() {
            std::fs::remove_dir_all(target)
        } else {
            std::fs::remove_file(target)
        };
        match r {
            Ok(_) => {
                results.push(item(p, "ok", None));
                mark_deleted(&app, p);
            }
            Err(e) => results.push(item(p, "failed", Some(e.to_string()))),
        }
    }
    Ok(results)
}

#[tauri::command]
pub fn action_issue_delete_token(paths: Vec<String>) -> Result<String, String> {
    let token = uuid_simple();
    let mut guard = TOKENS.lock().unwrap();
    guard.push((token.clone(), paths.clone(), Instant::now()));
    guard.retain(|(_, _, t)| t.elapsed() < Duration::from_secs(60));
    Ok(token)
}

fn verify_token(token: &str, paths: &[String]) -> bool {
    let mut guard = TOKENS.lock().unwrap();
    guard.retain(|(_, _, t)| t.elapsed() < Duration::from_secs(60));
    guard
        .iter()
        .any(|(t, ps, _)| t == token && ps.len() == paths.len() && ps.iter().zip(paths).all(|(a, b)| a == b))
}

static TOKENS: Mutex<Vec<(String, Vec<String>, Instant)>> = Mutex::new(Vec::new());

#[tauri::command]
pub fn action_open_with(path: String) -> Result<String, String> {
    shell_invoke(&path, "openas")?;
    Ok("已调出打开方式".into())
}

#[tauri::command]
pub fn action_properties(path: String) -> Result<String, String> {
    shell_invoke(&path, "properties")?;
    Ok("已调出属性".into())
}

#[tauri::command]
pub fn action_open_terminal(path: String) -> Result<String, String> {
    let dir = Path::new(&path);
    let dir = if dir.is_dir() {
        dir
    } else {
        dir.parent().unwrap_or(dir)
    };
    let dir_s = dir.to_string_lossy().to_string();
    if std::process::Command::new("wt").arg("-d").arg(&dir_s).spawn().is_err() {
        std::process::Command::new("cmd")
            .args(["/K", "cd", "/d", &dir_s])
            .spawn()
            .map_err(|e| format!("无法启动终端：{e}"))?;
    }
    Ok("已在终端打开".into())
}

#[tauri::command]
pub fn action_open_vscode(path: String) -> Result<String, String> {
    std::process::Command::new("cmd")
        .args(["/C", "code", &path])
        .spawn()
        .map(|_| "已在 VS Code 打开".to_string())
        .map_err(|_| "未检测到 code 命令，请安装 VS Code 并加入 PATH".into())
}

#[tauri::command]
pub fn action_rename(
    app: tauri::AppHandle,
    old_path: String,
    new_name: String,
) -> Result<String, String> {
    let old = Path::new(&old_path);
    let name = new_name.trim();
    if name.is_empty() || name.contains(['\\', '/', ':', '*', '?', '"', '<', '>', '|']) {
        return Err("文件名包含非法字符".into());
    }
    let new_path = old.with_file_name(name);
    if new_path.exists() {
        return Err("目标名称已存在".into());
    }
    std::fs::rename(old, &new_path).map_err(|e| format!("重命名失败：{e}"))?;
    let new_s = util::normalize_path(&new_path);
    let state: tauri::State<crate::AppState> = app.state();
    state.db.with_conn(|c| {
        c.execute(
            "UPDATE files SET path=?2, name=?3, updated_ms=?4 WHERE path=?1",
            params![old_path, new_s, name, util::now_ms()],
        )
        .ok();
        c.execute(
            "INSERT INTO index_outbox(file_id, operation, created_ms)
             SELECT file_id, 'upsert', ?2 FROM files WHERE path=?1",
            params![new_s, util::now_ms()],
        )
        .ok();
        Ok::<(), String>(())
    }).ok();
    Ok(new_s)
}

fn shell_invoke(path: &str, verb: &str) -> Result<(), String> {
    let path_h = HSTRING::from(path);
    let verb_h = HSTRING::from(verb);
    let mut sei = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_INVOKEIDLIST,
        lpVerb: PCWSTR(verb_h.as_ptr()),
        lpFile: PCWSTR(path_h.as_ptr()),
        nShow: SW_SHOW.0,
        ..Default::default()
    };
    unsafe { ShellExecuteExW(&mut sei) }.map_err(|e| format!("系统调用失败：{e}"))?;
    // HSTRING 需存活到调用结束
    drop(path_h);
    drop(verb_h);
    Ok(())
}

fn record_recent(app: &tauri::AppHandle, path: &str, action: &str) {
    let state: tauri::State<crate::AppState> = app.state();
    state
        .db
        .with_conn(|c| {
            let fid: Option<i64> = c
                .query_row("SELECT file_id FROM files WHERE path=?1", [path], |r| r.get(0))
                .ok();
            c.execute(
                "INSERT INTO recent_actions(file_id, path, action, happened_at) VALUES (?1,?2,?3,?4)",
                params![fid, path, action, util::now_rfc3339()],
            )
                .map_err(|e| e.to_string())?;
            c.execute(
                "DELETE FROM recent_actions WHERE happened_at < ?1",
                [(chrono::Utc::now() - chrono::Duration::days(90)).to_rfc3339()],
            )
                .map_err(|e| e.to_string())?;
            Ok::<(), String>(())
        })
        .ok();
}

fn mark_deleted(app: &tauri::AppHandle, path: &str) {
    let state: tauri::State<crate::AppState> = app.state();
    state
        .db
        .with_conn(|c| {
            let fid: Option<i64> = c
                .query_row("SELECT file_id FROM files WHERE path=?1", [path], |r| r.get(0))
                .ok();
            if let Some(fid) = fid {
                c.execute("DELETE FROM files WHERE file_id=?1", [fid]).ok();
                c.execute(
                    "INSERT INTO index_outbox(file_id, operation, created_ms) VALUES (?1,'delete',?2)",
                    params![fid, util::now_ms()],
                )
                .ok();
            }
            Ok::<(), String>(())
        })
        .ok();
}

fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{t:x}")
}
