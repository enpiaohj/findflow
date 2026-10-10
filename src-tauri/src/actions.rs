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
pub async fn action_open(app: tauri::AppHandle, path: String) -> Result<String, String> {
    use tauri_plugin_opener::OpenerExt;
    // 目标已被移动 / 删除，或在搜索框直接输入了不存在的路径时，给出明确提示，
    // 而不是交给系统弹出「找不到文件」对话框。放到阻塞线程：不可达的网络共享检查可能耗时数秒。
    let probe = path.clone();
    let exists = tauri::async_runtime::spawn_blocking(move || Path::new(&probe).exists())
        .await
        .map_err(|e| e.to_string())?;
    if !exists {
        return Err("路径不存在，可能已被移动或删除".into());
    }
    app.opener()
        .open_path(path.clone(), None::<&str>)
        .map_err(|e| format!("打开失败：{e}"))?;
    record_recent(&app, &path, "open");
    Ok("已打开".into())
}

/// 探测搜索框里键入的路径：存在则返回 `"dir"` / `"file"`，不存在返回 `None`。
/// 在阻塞线程里检查（不可达的网络共享可能耗时数秒，不能卡住界面线程）。
#[tauri::command]
pub async fn path_probe(path: String) -> Result<Option<String>, String> {
    let p = path.trim().trim_matches('"').to_string();
    if p.is_empty() {
        return Ok(None);
    }
    tauri::async_runtime::spawn_blocking(move || {
        std::fs::metadata(&p).ok().map(|m| if m.is_dir() { "dir" } else { "file" }.to_string())
    })
    .await
    .map_err(|e| e.to_string())
}

/// 在资源管理器中定位并选中该文件。
///
/// 不用 `explorer /select,<path>`：Explorer 会按逗号切分该参数，路径含逗号时
/// （如 `报告, 终稿.docx`）被拆成两段、解析为不存在的路径，于是静默打开「文档」
/// 目录而不是定位目标，用户只看到「定位没用」。这里改用系统 Shell API
/// `SHOpenFolderAndSelectItems`——资源管理器自身用的就是它，逗号、空格与 UNC
/// 路径都能正确处理，且会复用已打开的窗口。
#[tauri::command]
pub async fn action_locate(app: tauri::AppHandle, path: String) -> Result<String, String> {
    use tauri_plugin_opener::OpenerExt;
    // 该 API 要求路径存在；文件已被移动或删除时给出明确提示，
    // 而不是像 /select 那样悄悄打开一个无关目录。
    if !Path::new(&path).exists() {
        return Err("文件已不存在，可能已被移动或删除".into());
    }
    app.opener()
        .reveal_item_in_dir(Path::new(&path))
        .map_err(|e| format!("定位失败：{e}"))?;
    Ok("已定位".into())
}

#[tauri::command]
pub async fn action_copy_path(path: String, format: String) -> Result<String, String> {
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
pub async fn action_copy_files(paths: Vec<String>) -> Result<String, String> {
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
    #[allow(non_camel_case_types)]
    struct DropFiles {
        pfiles: u32,
        pt_x: i32,
        pt_y: i32,
        fnc: i32,
        fwide: i32,
    }
    let mut payload = vec![0u8; total];
    let df = DropFiles {
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

/// 删除目标：稳定 ID + 当前路径，供底线 3 双重校验。
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteTarget {
    pub file_id: i64,
    pub path: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteRequest {
    pub files: Vec<DeleteTarget>,
    pub token: String,
}

/// 底线 3：用稳定 ID 查库内当前路径，与传入路径比对。
/// 路径不一致 = 目标已被替换；查不到 = 已从索引移除。两者都拒绝执行。
pub(crate) fn verify_target(
    db: &crate::settings::SettingsStore,
    t: &DeleteTarget,
) -> Result<(), String> {
    let current: Option<String> = db.with_conn(|c| {
        c.query_row("SELECT path FROM files WHERE file_id=?1", [t.file_id], |r| r.get(0))
            .ok()
    });
    match current {
        Some(p) if p == t.path => Ok(()),
        Some(_) => Err("目标已变更，请重新确认".into()),
        None => Err("目标已不在索引中，请刷新后重试".into()),
    }
}

#[tauri::command]
pub async fn action_recycle(
    app: tauri::AppHandle,
    request: DeleteRequest,
) -> Result<Vec<crate::models::PlanItem>, String> {
    let state: tauri::State<std::sync::Arc<crate::AppState>> = app.state();
    let fp = delete_fingerprint(&request.files);
    if !consume_token(&request.token, &fp) {
        return Err("删除确认令牌无效或已过期，请重新确认".into());
    }
    let mut results = Vec::new();
    for t in &request.files {
        if let Err(reason) = verify_target(&state.db, t) {
            results.push(item(&t.path, "stale", Some(reason)));
            continue;
        }
        let target = Path::new(&t.path);
        if !target.exists() {
            results.push(item(&t.path, "missing", Some("目标不存在".into())));
            continue;
        }
        match trash::delete(target) {
            Ok(_) => {
                results.push(item(&t.path, "ok", None));
                mark_deleted(&app, &t.path);
            }
            Err(e) => results.push(item(&t.path, "failed", Some(e.to_string()))),
        }
    }
    Ok(results)
}

#[tauri::command]
pub async fn action_delete_permanent(
    app: tauri::AppHandle,
    request: DeleteRequest,
) -> Result<Vec<crate::models::PlanItem>, String> {
    let state: tauri::State<std::sync::Arc<crate::AppState>> = app.state();
    let fp = delete_fingerprint(&request.files);
    if !consume_token(&request.token, &fp) {
        return Err("删除确认令牌无效或已过期，请重新确认".into());
    }
    let mut results = Vec::new();
    for t in &request.files {
        if let Err(reason) = verify_target(&state.db, t) {
            results.push(item(&t.path, "stale", Some(reason)));
            continue;
        }
        let target = Path::new(&t.path);
        if !target.exists() {
            results.push(item(&t.path, "missing", Some("目标不存在".into())));
            continue;
        }
        let r = if target.is_dir() {
            std::fs::remove_dir_all(target)
        } else {
            std::fs::remove_file(target)
        };
        match r {
            Ok(_) => {
                results.push(item(&t.path, "ok", None));
                mark_deleted(&app, &t.path);
            }
            Err(e) => results.push(item(&t.path, "failed", Some(e.to_string()))),
        }
    }
    Ok(results)
}

/// 删除请求指纹：逐项「fileId + 路径」拼接，令牌据此绑定具体目标。
fn delete_fingerprint(files: &[DeleteTarget]) -> String {
    let parts: Vec<String> = files
        .iter()
        .map(|f| format!("{}:{}", f.file_id, f.path))
        .collect();
    parts.join("\u{1}")
}

#[tauri::command]
pub async fn action_issue_delete_token(files: Vec<DeleteTarget>) -> Result<String, String> {
    issue_token(&delete_fingerprint(&files))
}

/// 请求指纹：用不可出现在路径或规则名中的分隔符拼接，供令牌绑定。
/// 删除 → 逐项「fileId:path」；整理 → 「来源目录 / 规则 / 目标目录」。
pub(crate) fn fingerprint(parts: &[&str]) -> String {
    parts.join("\u{1}")
}

/// 签发一次性令牌（60 秒有效）。
pub(crate) fn issue_token(fp: &str) -> Result<String, String> {
    let token = new_token()?;
    let mut guard = TOKENS.lock().unwrap();
    guard.retain(|(_, _, t)| t.elapsed() < Duration::from_secs(60));
    guard.push((token.clone(), fp.to_string(), Instant::now()));
    Ok(token)
}

/// 校验并消费令牌：命中即移除，确保不可重放。
pub(crate) fn consume_token(token: &str, fp: &str) -> bool {
    let mut guard = TOKENS.lock().unwrap();
    guard.retain(|(_, _, t)| t.elapsed() < Duration::from_secs(60));
    let Some(pos) = guard.iter().position(|(t, f, _)| t == token && f == fp) else {
        return false;
    };
    guard.remove(pos);
    true
}

/// 密码学安全随机令牌（BCryptGenRandom，系统首选 RNG），256 位十六进制。
fn new_token() -> Result<String, String> {
    use windows::Win32::Security::Cryptography::{
        BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG,
    };
    let mut buf = [0u8; 32];
    // halgorithm 传 None 即使用系统首选 RNG。
    let status = unsafe { BCryptGenRandom(None, &mut buf, BCRYPT_USE_SYSTEM_PREFERRED_RNG) };
    if status.is_err() {
        return Err("生成确认令牌失败".into());
    }
    Ok(buf.iter().map(|b| format!("{b:02x}")).collect())
}

static TOKENS: Mutex<Vec<(String, String, Instant)>> = Mutex::new(Vec::new());

#[tauri::command]
pub async fn action_open_with(path: String) -> Result<String, String> {
    shell_invoke(&path, "openas")?;
    Ok("已调出打开方式".into())
}

#[tauri::command]
pub async fn action_properties(path: String) -> Result<String, String> {
    shell_invoke(&path, "properties")?;
    Ok("已调出属性".into())
}

#[tauri::command]
pub async fn action_open_terminal(path: String) -> Result<String, String> {
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
pub async fn action_open_vscode(path: String) -> Result<String, String> {
    // 必须等 cmd 结束并看退出码：spawn() 只要 cmd.exe 存在就成功，而 `code`
    // 缺失时失败发生在 cmd 内部，原先只看 spawn 会把失败一律报成「已在 VS Code 打开」。
    // code 是 .cmd 批处理，无法由 CreateProcessW 直接按 PATH 解析，故仍经 cmd 调用。
    let status = tauri::async_runtime::spawn_blocking(move || {
        std::process::Command::new("cmd")
            .args(["/C", "code", &path])
            .status()
    })
    .await
    .map_err(|e| format!("无法启动 VS Code：{e}"))?
    .map_err(|e| format!("无法启动 VS Code：{e}"))?;
    if !status.success() {
        return Err("未检测到 code 命令，请安装 VS Code 并加入 PATH".into());
    }
    Ok("已在 VS Code 打开".into())
}

#[tauri::command]
pub async fn action_rename(
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
    // 必须在 rename 之前判断：rename 之后旧路径已不存在，is_dir 恒为 false
    let was_dir = old.is_dir();
    std::fs::rename(old, &new_path).map_err(|e| format!("重命名失败：{e}"))?;
    let new_s = util::normalize_path(&new_path);
    let state: tauri::State<std::sync::Arc<crate::AppState>> = app.state();
    state
        .db
        .with_conn(|c| {
            rename_db_rows(c, &old_path, &new_s, name, was_dir);
            Ok::<(), String>(())
        })
        .ok();
    Ok(new_s)
}

/// 重命名后同步数据库。
///
/// 目录重命名时子树路径要整体替换前缀：漏掉这步会让目录内所有文件在库里保留旧路径，
/// 搜索命中后打开与定位都会指向不存在的位置（watcher.rs 的目录重命名处理与此一致）。
/// 子项文件名不变，故无需重建索引文档。
///
/// 单独抽出是因为 `action_rename` 需要 `AppHandle`，无法直接单测。
fn rename_db_rows(
    c: &rusqlite::Connection,
    old_path: &str,
    new_path: &str,
    name: &str,
    was_dir: bool,
) {
    let now = util::now_ms();
    c.execute(
        "UPDATE files SET path=?2, name=?3, updated_ms=?4 WHERE path=?1",
        params![old_path, new_path, name, now],
    )
    .ok();
    if was_dir {
        let (lo, hi) = util::subtree_bounds(old_path);
        c.execute(
            "UPDATE files SET path = ?1 || substr(path, length(?2) + 1),
                              parent_path = ?1 || substr(parent_path, length(?2) + 1),
                              updated_ms = ?5
             WHERE path>=?3 AND path<?4",
            params![new_path, old_path, lo, hi, now],
        )
        .ok();
    }
    c.execute(
        "INSERT INTO index_outbox(file_id, operation, created_ms)
         SELECT file_id, 'upsert', ?2 FROM files WHERE path=?1",
        params![new_path, now],
    )
    .ok();
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
    let state: tauri::State<std::sync::Arc<crate::AppState>> = app.state();
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
    let state: tauri::State<std::sync::Arc<crate::AppState>> = app.state();
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

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> crate::settings::SettingsStore {
        crate::settings::SettingsStore::new(crate::db::open_in_memory().unwrap())
    }

    #[test]
    fn token_is_random_and_unique() {
        let mut seen = std::collections::HashSet::new();
        for _ in 0..100 {
            let t = new_token().unwrap();
            assert_eq!(t.len(), 64, "256 位应为 64 个十六进制字符");
            assert!(t.chars().all(|c| c.is_ascii_hexdigit()));
            assert!(seen.insert(t), "令牌必须互不相同");
        }
    }

    #[test]
    fn consume_token_rejects_replay() {
        let fp = fingerprint(&["D:\\下载", "extension", "D:\\整理"]);
        let token = issue_token(&fp).unwrap();
        assert!(consume_token(&token, &fp), "首次消费应成功");
        assert!(!consume_token(&token, &fp), "同一令牌不可重放");
    }

    #[test]
    fn consume_token_rejects_mismatch() {
        let fp = fingerprint(&["D:\\下载", "extension", "D:\\整理"]);
        let token = issue_token(&fp).unwrap();
        // 指纹不符（换了目标目录）→ 拒绝，且不消费
        assert!(!consume_token(
            &token,
            &fingerprint(&["D:\\下载", "extension", "D:\\其它"])
        ));
        // 原指纹仍可用
        assert!(consume_token(&token, &fp));
    }

    #[test]
    fn delete_fingerprint_binds_id_and_path() {
        let a = DeleteTarget { file_id: 1, path: "D:\\a.txt".into() };
        let b = DeleteTarget { file_id: 2, path: "D:\\a.txt".into() };
        let c = DeleteTarget { file_id: 1, path: "D:\\b.txt".into() };
        let fp = delete_fingerprint(std::slice::from_ref(&a));
        assert_ne!(fp, delete_fingerprint(std::slice::from_ref(&b)), "fileId 变化应改变指纹");
        assert_ne!(fp, delete_fingerprint(std::slice::from_ref(&c)), "路径变化应改变指纹");
        let both = delete_fingerprint(&[a.clone(), b.clone()]);
        assert_ne!(both, delete_fingerprint(&[b, a]), "顺序应影响指纹");
    }

    #[test]
    fn verify_target_detects_replacement() {
        let s = store();
        s.with_conn(|c| {
            c.execute(
                "INSERT INTO scopes(scope_id, path, added_at) VALUES (1,'D:\\s','now')",
                [],
            )
            .unwrap();
            c.execute(
                "INSERT INTO files(file_id, scope_id, name, path, ext, kind, size, is_dir, updated_ms)
                 VALUES (7, 1, 'a.txt', 'D:\\s\\a.txt', 'txt', 'doc', 1, 0, 1)",
                [],
            )
            .unwrap();
            Ok::<(), String>(())
        })
        .unwrap();

        // 路径一致 → 通过
        let ok = DeleteTarget { file_id: 7, path: "D:\\s\\a.txt".into() };
        assert!(verify_target(&s, &ok).is_ok());

        // 同一 fileId 但路径已变（目标被替换）→ 拒绝
        let moved = DeleteTarget { file_id: 7, path: "D:\\s\\renamed.txt".into() };
        assert!(verify_target(&s, &moved).is_err());

        // fileId 不在索引中 → 拒绝
        let gone = DeleteTarget { file_id: 999, path: "D:\\s\\a.txt".into() };
        assert!(verify_target(&s, &gone).is_err());
    }

    /// 往库里塞一批行（scope 1），模拟一个已建索引的目录树。
    /// 入参：(名称, 完整路径, 父目录, 是否目录)。
    fn seed(s: &crate::settings::SettingsStore, rows: &[(&str, &str, &str, bool)]) {
        s.with_conn(|c| {
            c.execute(
                "INSERT INTO scopes(scope_id, path, added_at) VALUES (1,'D:\\s','now')",
                [],
            )
            .unwrap();
            for (name, path, parent, is_dir) in rows {
                c.execute(
                    "INSERT INTO files(scope_id, parent_path, name, path, ext, kind, size, is_dir, updated_ms)
                     VALUES (1, ?1, ?2, ?3, '', ?4, 1, ?5, 1)",
                    params![
                        parent,
                        name,
                        path,
                        if *is_dir { "folder" } else { "doc" },
                        *is_dir as i64
                    ],
                )
                .unwrap();
            }
            Ok::<(), String>(())
        })
        .unwrap();
    }

    fn path_of(c: &rusqlite::Connection, name: &str) -> String {
        c.query_row("SELECT path FROM files WHERE name=?1", [name], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn rename_directory_updates_descendant_paths() {
        let s = store();
        seed(
            &s,
            &[
                ("旧名", r"D:\s\旧名", r"D:\s", true),
                ("a.txt", r"D:\s\旧名\a.txt", r"D:\s\旧名", false),
                ("b.txt", r"D:\s\旧名\sub\b.txt", r"D:\s\旧名\sub", false),
                // 前缀相似但不是子树的兄弟目录，不得被改写
                ("c.txt", r"D:\s\旧名字典\c.txt", r"D:\s\旧名字典", false),
            ],
        );
        s.with_conn(|c| {
            rename_db_rows(c, r"D:\s\旧名", r"D:\s\新名", "新名", true);
            assert_eq!(path_of(c, "新名"), r"D:\s\新名");
            assert_eq!(path_of(c, "a.txt"), r"D:\s\新名\a.txt");
            // 多级子目录同样跟随
            assert_eq!(path_of(c, "b.txt"), r"D:\s\新名\sub\b.txt");
            // 前缀相似的兄弟目录不得被误改
            assert_eq!(path_of(c, "c.txt"), r"D:\s\旧名字典\c.txt");
            let parent: String = c
                .query_row("SELECT parent_path FROM files WHERE name='a.txt'", [], |r| r.get(0))
                .unwrap();
            assert_eq!(parent, r"D:\s\新名");
            Ok::<(), String>(())
        })
        .unwrap();
    }

    #[test]
    fn rename_file_leaves_siblings_untouched() {
        let s = store();
        seed(
            &s,
            &[
                ("a.txt", r"D:\s\a.txt", r"D:\s", false),
                ("a.txt.bak", r"D:\s\a.txt.bak", r"D:\s", false),
            ],
        );
        s.with_conn(|c| {
            // 重命名单个文件（非目录）不得触发子树前缀替换
            rename_db_rows(c, r"D:\s\a.txt", r"D:\s\z.txt", "z.txt", false);
            assert_eq!(path_of(c, "z.txt"), r"D:\s\z.txt");
            assert_eq!(path_of(c, "a.txt.bak"), r"D:\s\a.txt.bak");
            Ok::<(), String>(())
        })
        .unwrap();
    }
}
