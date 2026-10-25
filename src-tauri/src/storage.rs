//! 启动前的数据目录检查。位置配置独立于数据目录，损坏的 junction 不会阻断恢复。
use std::path::{Path, PathBuf};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Location {
    directory: PathBuf,
    #[serde(default)]
    pending: Option<PathBuf>,
}

pub fn config_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    use tauri::Manager;
    Ok(app.path().config_dir().map_err(|e| e.to_string())?
        .join(format!("{}.storage", app.config().identifier)).join("location.json"))
}

pub fn same_directory(a: &Path, b: &Path) -> Result<bool, String> {
    let a = std::fs::canonicalize(a).map_err(|e| e.to_string())?;
    let b = std::fs::canonicalize(b).map_err(|e| e.to_string())?;
    Ok(a == b)
}

fn check_directory(dir: &Path) -> Result<(), String> {
    // symlink_metadata 能识别目标已经丢失的 junction；不能把它当首次启动。
    std::fs::symlink_metadata(dir).map_err(|e| format!("数据目录不存在或无法访问 {}：{e}", dir.display()))?;
    if !dir.is_dir() {
        return Err(format!("数据目录或目录链接目标不可用：{}", dir.display()));
    }
    Ok(())
}

pub fn validate_existing(dir: &Path) -> Result<(), String> {
    validate_database(dir, true)
}

fn validate_database(dir: &Path, thorough: bool) -> Result<(), String> {
    check_directory(dir)?;
    let conn = Connection::open_with_flags(dir.join("findflow.db"), OpenFlags::SQLITE_OPEN_READ_WRITE)
        .map_err(|e| format!("无法读取已有数据库（不会创建空库）：{e}"))?;
    conn.busy_timeout(std::time::Duration::from_millis(500)).map_err(|e| e.to_string())?;
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).map_err(|e| e.to_string())?;
    if !crate::db::supported_version(version) {
        return Err(format!("数据库版本 {version} 不是可接入的 FindFlow 数据库"));
    }
    for query in [
        "SELECT key, value_json, updated_at FROM settings LIMIT 0",
        "SELECT scope_id, path, enabled FROM scopes LIMIT 0",
        "SELECT file_id, path, name FROM files LIMIT 0",
        "SELECT file_id, operation FROM index_outbox LIMIT 0",
    ] {
        conn.prepare(query).map_err(|e| format!("数据库结构不兼容：{e}"))?;
    }
    conn.query_row("SELECT COUNT(*) FROM (SELECT file_id FROM files LIMIT 1)", [], |r| r.get::<_, i64>(0)).map_err(|e| format!("数据库不可读：{e}"))?;
    if thorough {
        crate::db::validate_schema(&conn, version)?;
        let check: String = conn.query_row("PRAGMA quick_check", [], |r| r.get(0)).map_err(|e| e.to_string())?;
        if check != "ok" { return Err(format!("数据库检查失败：{check}；请重新选择数据目录或从备份恢复")); }
    }
    conn.execute_batch("BEGIN IMMEDIATE; ROLLBACK;")
        .map_err(|e| format!("数据库被占用或无法写入，请关闭占用程序后重试：{e}"))?;
    Ok(())
}

fn save(marker: &Path, location: &Location) -> Result<(), String> {
    let parent = marker.parent().ok_or("配置路径无效")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let tmp = parent.join(format!("location-{}-{}.tmp", std::process::id(), crate::util::now_ms()));
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&tmp).map_err(|e| e.to_string())?;
    let result = (|| {
        file.write_all(&serde_json::to_vec(location).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            use windows::core::PCWSTR;
            use windows::Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH};
            let from: Vec<u16> = tmp.as_os_str().encode_wide().chain(Some(0)).collect();
            let to: Vec<u16> = marker.as_os_str().encode_wide().chain(Some(0)).collect();
            unsafe { MoveFileExW(PCWSTR(from.as_ptr()), PCWSTR(to.as_ptr()), MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH) }.map_err(|e| e.to_string())?;
        }
        #[cfg(not(windows))]
        std::fs::rename(&tmp, marker).map_err(|e| e.to_string())?;
        Ok(())
    })();
    if result.is_err() { let _ = std::fs::remove_file(tmp); }
    result
}

/// 在数据库创建/迁移成功后记录入口；不能吞掉尚未执行的迁移计划。
pub fn remember_initialized(marker: &Path, dir: &Path) -> Result<(), String> {
    validate_database(dir, false)?;
    let directory = std::fs::canonicalize(dir).map_err(|e| e.to_string())?;
    match std::fs::read(marker) {
        Ok(bytes) => {
            let existing: Location = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            if existing.pending.is_some() {
                return Err("仍有待执行的迁移计划，不能覆盖数据位置".into());
            }
            if existing.directory == directory { return Ok(()); }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
        Err(e) => return Err(e.to_string()),
    }
    save(marker, &Location { directory, pending: None })
}

pub fn select_existing(marker: &Path, dir: &Path) -> Result<(), String> {
    validate_existing(dir)?;
    save(marker, &Location { directory: std::fs::canonicalize(dir).map_err(|e| e.to_string())?, pending: None })
}

fn read_location(marker: &Path, default: &Path) -> Result<Option<Location>, String> {
    match std::fs::read(marker) {
        Ok(bytes) => return serde_json::from_slice(&bytes).map(Some).map_err(|e| format!("数据位置配置损坏：{e}")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
        Err(e) => return Err(e.to_string()),
    }
    // 旧版本在重启前只保存目标路径；空目标且默认库仍在表示待迁移计划。
    match std::fs::read_to_string(default.join("storage-location.txt")) {
        Ok(s) if !s.trim().is_empty() => {
            let target = PathBuf::from(s.trim());
            if !target.join("findflow.db").try_exists().map_err(|e| e.to_string())?
                && default.join("findflow.db").try_exists().map_err(|e| e.to_string())?
            {
                schedule(marker, default, &target)?;
                return Ok(Some(Location {
                    directory: std::fs::canonicalize(default).map_err(|e| e.to_string())?,
                    pending: Some(std::fs::canonicalize(target).map_err(|e| e.to_string())?),
                }));
            }
            Ok(Some(Location { directory: target, pending: None }))
        },
        Ok(_) => Ok(None),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("无法读取旧数据位置配置：{e}")),
    }
}

pub fn prepare(marker: &Path, default: &Path) -> Result<PathBuf, String> {
    if let Some(location) = read_location(marker, default)? {
        validate_database(&location.directory, location.pending.is_some())?;
        if let Some(target) = location.pending {
            migrate(marker, &location.directory, &target)?;
            return Ok(target);
        }
        return Ok(location.directory);
    }
    match std::fs::symlink_metadata(default) {
        Ok(_) => {
            check_directory(default)?;
            if default.join("findflow.db").try_exists().map_err(|e| e.to_string())? {
                validate_database(default, false)?;
            } else if std::fs::read_dir(default).map_err(|e| e.to_string())?.next().is_some() {
                return Err("数据目录已有内容但缺少数据库，请选择原数据目录；不会创建空库".into());
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => std::fs::create_dir_all(default).map_err(|e| e.to_string())?,
        Err(e) => return Err(e.to_string()),
    }
    Ok(default.to_path_buf())
}

fn empty_target(target: &Path) -> Result<(), String> {
    check_directory(target)?;
    if std::fs::read_dir(target).map_err(|e| e.to_string())?.next().is_some() {
        return Err("迁移目标必须为空目录；接入已有数据库请在启动恢复窗口中选择".into());
    }
    Ok(())
}

pub fn schedule(marker: &Path, source: &Path, target: &Path) -> Result<(), String> {
    validate_existing(source)?;
    check_directory(target)?;
    if same_directory(source, target)? { return Err("新位置与当前数据目录相同（包括目录链接）".into()); }
    let source = std::fs::canonicalize(source).map_err(|e| e.to_string())?;
    let target = std::fs::canonicalize(target).map_err(|e| e.to_string())?;
    if target.starts_with(&source) || source.starts_with(&target) { return Err("源目录与目标目录不能互相包含".into()); }
    empty_target(&target)?;
    save(marker, &Location { directory: source, pending: Some(target) })
}

fn migrate(marker: &Path, source: &Path, target: &Path) -> Result<(), String> {
    let resolved_source = std::fs::canonicalize(source).map_err(|e| e.to_string())?;
    let resolved_target = std::fs::canonicalize(target).map_err(|e| e.to_string())?;
    // 计划中保存的路径已经 canonicalize；重启前被替换为 junction 时拒绝继续。
    if resolved_source != source || resolved_target != target {
        return Err("迁移目录入口已发生变化，请重新选择目录并保存迁移计划".into());
    }
    if resolved_target.starts_with(&resolved_source) || resolved_source.starts_with(&resolved_target) {
        return Err("迁移源与目标相同或互相包含，已停止迁移".into());
    }
    empty_target(target)?;
    // VACUUM INTO 包含已提交的 WAL 内容，不复制活动的 db/wal/shm 三件套。
    // 失败时源数据库始终保留；目标残留也保留供检查，不能误报切换成功。
    let guard = Connection::open_with_flags(source.join("findflow.db"), OpenFlags::SQLITE_OPEN_READ_WRITE).map_err(|e| e.to_string())?;
    guard.execute_batch("BEGIN IMMEDIATE").map_err(|e| format!("源数据库被占用，停止迁移：{e}"))?;
    let conn = Connection::open_with_flags(source.join("findflow.db"), OpenFlags::SQLITE_OPEN_READ_WRITE).map_err(|e| e.to_string())?;
    conn.execute("VACUUM INTO ?1", [target.join("findflow.db").to_string_lossy().as_ref()]).map_err(|e| format!("迁移失败，原数据库已保留：{e}"))?;
    validate_existing(target)?;
    save(marker, &Location { directory: target.to_path_buf(), pending: None })
}

/// 返回 1=选择、2=重试、0=退出；启动阶段不依赖 Tauri 事件循环。
pub fn recovery_action(message: &str) -> u8 {
    #[cfg(windows)]
    {
        use windows::core::PCWSTR;
        use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_YESNOCANCEL, MB_ICONERROR, IDYES, IDNO};
        let text: Vec<u16> = message.encode_utf16().chain(Some(0)).collect();
        let title: Vec<u16> = "FindFlow 数据恢复".encode_utf16().chain(Some(0)).collect();
        let result = unsafe { MessageBoxW(None, PCWSTR(text.as_ptr()), PCWSTR(title.as_ptr()), MB_YESNOCANCEL | MB_ICONERROR) };
        if result == IDYES { 1 } else if result == IDNO { 2 } else { 0 }
    }
    #[cfg(not(windows))]
    { eprintln!("{message}"); 0 }
}

pub fn pick_existing_folder() -> Result<Option<PathBuf>, String> {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, CoCreateInstance, CoTaskMemFree, COINIT_APARTMENTTHREADED, CLSCTX_INPROC_SERVER};
        use windows::Win32::UI::Shell::{FileOpenDialog, IFileOpenDialog, FOS_PICKFOLDERS, FOS_FORCEFILESYSTEM, FOS_PATHMUSTEXIST, SIGDN_FILESYSPATH};
        use windows::core::w;
        let initialized = CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok();
        let result = (|| {
            let dialog: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).map_err(|e| e.to_string())?;
            dialog.SetOptions(dialog.GetOptions().map_err(|e| e.to_string())? | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST).map_err(|e| e.to_string())?;
            dialog.SetTitle(w!("选择包含 findflow.db 的已有数据目录")).map_err(|e| e.to_string())?;
            if let Err(error) = dialog.Show(None) {
                if error.code().0 as u32 == 0x800704C7 { return Ok(None); }
                return Err(error.to_string());
            }
            let item = dialog.GetResult().map_err(|e| e.to_string())?;
            let name = item.GetDisplayName(SIGDN_FILESYSPATH).map_err(|e| e.to_string())?;
            let path = name.to_string().map_err(|e| e.to_string());
            CoTaskMemFree(Some(name.0.cast()));
            Ok(Some(PathBuf::from(path?)))
        })();
        if initialized { CoUninitialize(); }
        result
    }
    #[cfg(not(windows))]
    { Ok(None) }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn database(dir: &Path) { crate::db::open(&dir.join("findflow.db")).unwrap(); }
    #[test]
    fn initialized_location_survives_missing_data_and_preserves_pending() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source"); std::fs::create_dir(&source).unwrap(); database(&source);
        let marker = temp.path().join("config/location.json");
        remember_initialized(&marker, &source).unwrap();
        let target = temp.path().join("target"); std::fs::create_dir(&target).unwrap();
        schedule(&marker, &source, &target).unwrap();
        let before = std::fs::read(&marker).unwrap();
        assert!(remember_initialized(&marker, &source).is_err());
        assert_eq!(std::fs::read(&marker).unwrap(), before);
        std::fs::rename(&source, temp.path().join("saved-source")).unwrap();
        assert!(prepare(&marker, &source).is_err());
        assert!(!source.exists());
    }
    #[test]
    fn missing_custom_directory_never_creates_database() {
        let temp = tempfile::tempdir().unwrap();
        let marker = temp.path().join("config/location.json");
        let missing = temp.path().join("missing");
        save(&marker, &Location { directory: missing.clone(), pending: None }).unwrap();
        assert!(prepare(&marker, &temp.path().join("default")).is_err());
        assert!(!missing.exists());
    }
    #[test]
    fn unrelated_or_corrupt_database_rejected() {
        let temp = tempfile::tempdir().unwrap();
        Connection::open(temp.path().join("findflow.db")).unwrap();
        assert!(validate_existing(temp.path()).is_err());
        std::fs::write(temp.path().join("findflow.db"), b"corrupt").unwrap();
        assert!(validate_existing(temp.path()).is_err());
    }
    #[test]
    fn migration_includes_wal_and_preserves_source() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source"); let target = temp.path().join("target");
        std::fs::create_dir(&source).unwrap(); std::fs::create_dir(&target).unwrap();
        let conn = crate::db::open(&source.join("findflow.db")).unwrap();
        conn.execute("INSERT INTO settings VALUES ('probe', 'true', 'now')", []).unwrap();
        let marker = temp.path().join("config/location.json");
        schedule(&marker, &source, &target).unwrap();
        assert_eq!(prepare(&marker, &source).unwrap(), std::fs::canonicalize(&target).unwrap());
        let copied = Connection::open(target.join("findflow.db")).unwrap();
        assert_eq!(copied.query_row("SELECT value_json FROM settings WHERE key='probe'", [], |r| r.get::<_, String>(0)).unwrap(), "true");
        assert!(source.join("findflow.db").exists());
    }
    #[test]
    fn occupied_target_and_same_directory_rejected() {
        let temp = tempfile::tempdir().unwrap(); database(temp.path());
        let marker = temp.path().join("location.json");
        assert!(schedule(&marker, temp.path(), temp.path()).is_err());
        let target = tempfile::tempdir().unwrap(); std::fs::write(target.path().join("keep"), "data").unwrap();
        assert!(schedule(&marker, temp.path(), target.path()).is_err());
        assert_eq!(std::fs::read_to_string(target.path().join("keep")).unwrap(), "data");
    }
    #[test]
    fn stable_configuration_recovers_when_default_is_a_file() {
        let temp = tempfile::tempdir().unwrap();
        let default = temp.path().join("broken-default"); std::fs::write(&default, "blocked").unwrap();
        let data = tempfile::tempdir().unwrap(); database(data.path());
        let marker = temp.path().join("config/location.json");
        select_existing(&marker, data.path()).unwrap();
        assert_eq!(prepare(&marker, &default).unwrap(), std::fs::canonicalize(data.path()).unwrap());
    }
    #[cfg(windows)]
    #[test]
    fn junction_alias_and_broken_target_are_safe() {
        let temp = tempfile::tempdir().unwrap();
        let data = temp.path().join("data"); std::fs::create_dir(&data).unwrap(); database(&data);
        let alias = temp.path().join("alias");
        let result = std::process::Command::new("cmd").args(["/C", "mklink", "/J"]).arg(&alias).arg(&data).output().unwrap();
        assert!(result.status.success());
        assert!(same_directory(&alias, &data).unwrap());
        assert!(schedule(&temp.path().join("config/location.json"), &data, &alias).is_err());
        std::fs::remove_dir_all(&data).unwrap();
        assert!(std::fs::symlink_metadata(&alias).is_ok());
        assert!(prepare(&temp.path().join("config/location.json"), &alias).is_err());
        assert!(!data.exists());
        std::fs::remove_dir(&alias).unwrap();
    }
    #[test]
    fn stable_config_bypasses_unavailable_default() {
        let temp = tempfile::tempdir().unwrap(); database(temp.path());
        let marker = temp.path().join("config/location.json");
        select_existing(&marker, temp.path()).unwrap();
        let blocked = temp.path().join("blocked"); std::fs::write(&blocked, "not a directory").unwrap();
        assert_eq!(prepare(&marker, &blocked).unwrap(), std::fs::canonicalize(temp.path()).unwrap());
    }
    #[test]
    fn failed_atomic_save_preserves_existing_marker() {
        let temp = tempfile::tempdir().unwrap(); database(temp.path());
        let marker = temp.path().join("marker"); std::fs::create_dir(&marker).unwrap();
        std::fs::write(marker.join("keep"), "original").unwrap();
        assert!(select_existing(&marker, temp.path()).is_err());
        assert_eq!(std::fs::read_to_string(marker.join("keep")).unwrap(), "original");
    }
    #[cfg(windows)]
    #[test]
    fn junction_alias_and_broken_target_are_not_empty_first_launch() {
        let temp = tempfile::tempdir().unwrap();
        let original = temp.path().join("original"); std::fs::create_dir(&original).unwrap(); database(&original);
        let alias = temp.path().join("alias");
        let status = std::process::Command::new("cmd").args(["/C", "mklink", "/J"]).arg(&alias).arg(&original).output().unwrap();
        assert!(status.status.success(), "junction 创建失败");
        assert!(same_directory(&alias, &original).unwrap());
        assert!(schedule(&temp.path().join("location.json"), &original, &alias).is_err());
        let renamed = temp.path().join("renamed"); std::fs::rename(&original, &renamed).unwrap();
        assert!(std::fs::symlink_metadata(&alias).is_ok());
        assert!(prepare(&temp.path().join("absent.json"), &alias).is_err());
        select_existing(&temp.path().join("stable/location.json"), &renamed).unwrap();
        assert_eq!(prepare(&temp.path().join("stable/location.json"), &alias).unwrap(), std::fs::canonicalize(&renamed).unwrap());
        std::fs::remove_dir(&alias).unwrap();
    }
    #[test]
    fn legacy_empty_destination_migrates_without_losing_source() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source"); let target = temp.path().join("target");
        std::fs::create_dir(&source).unwrap(); std::fs::create_dir(&target).unwrap();
        database(&source);
        std::fs::write(source.join("storage-location.txt"), target.to_string_lossy().as_bytes()).unwrap();
        let marker = temp.path().join("config/location.json");
        assert_eq!(prepare(&marker, &source).unwrap(), std::fs::canonicalize(&target).unwrap());
        assert!(source.join("findflow.db").exists());
        validate_existing(&target).unwrap();
    }
    #[test]
    fn legacy_occupied_destination_is_preserved() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source"); let target = temp.path().join("target");
        std::fs::create_dir(&source).unwrap(); std::fs::create_dir(&target).unwrap(); database(&source);
        std::fs::write(target.join("keep"), "asset").unwrap();
        std::fs::write(source.join("storage-location.txt"), target.to_string_lossy().as_bytes()).unwrap();
        assert!(prepare(&temp.path().join("location.json"), &source).is_err());
        assert_eq!(std::fs::read_to_string(target.join("keep")).unwrap(), "asset");
    }
    #[test]
    fn restore_rejects_missing_business_table_and_column() {
        let temp = tempfile::tempdir().unwrap(); database(temp.path());
        let conn = Connection::open(temp.path().join("findflow.db")).unwrap();
        conn.execute_batch("ALTER TABLE jobs DROP COLUMN checkpoint_json").unwrap();
        assert!(validate_existing(temp.path()).unwrap_err().contains("jobs"));
        drop(conn);
        let other = tempfile::tempdir().unwrap(); database(other.path());
        let conn = Connection::open(other.path().join("findflow.db")).unwrap();
        conn.execute_batch("DROP TABLE favorites").unwrap();
        assert!(validate_existing(other.path()).unwrap_err().contains("favorites"));
    }
    #[test]
    fn unknown_directory_without_database_stays_blocked() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir(temp.path().join("logs")).unwrap();
        assert!(prepare(&temp.path().join("absent/location.json"), temp.path()).is_err());
        assert!(!temp.path().join("findflow.db").exists());
    }
    #[test]
    fn locked_database_rejected() {
        let temp = tempfile::tempdir().unwrap(); let conn = crate::db::open(&temp.path().join("findflow.db")).unwrap();
        conn.execute_batch("BEGIN IMMEDIATE").unwrap();
        assert!(validate_existing(temp.path()).is_err());
    }
}
