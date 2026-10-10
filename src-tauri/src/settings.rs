use std::sync::Mutex;

use rusqlite::Connection;
use serde_json::Value;
use tauri::State;

/// 后台专用数据库连接句柄（与界面连接分离，避免互相阻塞）。
pub struct DbHandle(pub Mutex<Connection>);

impl DbHandle {
    pub fn new(conn: Connection) -> Self {
        Self(Mutex::new(conn))
    }

    pub fn with_conn<T>(&self, f: impl FnOnce(&Connection) -> T) -> T {
        let g = self.0.lock().unwrap();
        f(&g)
    }
}

/// 全局设置存储：key -> JSON 值，SQLite 单连接互斥复用。
pub struct SettingsStore {
    conn: Mutex<Connection>,
    /// 实例唯一标识：供进程级缓存（如索引总数）区分不同数据库，
    /// 避免同一进程内多个库（测试内存库、切换数据目录）互相命中缓存。
    id: usize,
}

impl SettingsStore {
    pub fn new(conn: Connection) -> Self {
        static NEXT_ID: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);
        Self {
            conn: Mutex::new(conn),
            id: NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        }
    }

    /// 本实例的唯一标识，用于区分不同数据库的缓存条目。
    pub fn id(&self) -> usize {
        self.id
    }

    pub fn get(&self, key: &str) -> Result<Option<Value>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT value_json FROM settings WHERE key = ?1")?;
        let mut rows = stmt.query([key])?;
        match rows.next()? {
            Some(row) => {
                let raw: String = row.get(0)?;
                Ok(serde_json::from_str(&raw).ok())
            }
            None => Ok(None),
        }
    }

    pub fn get_or(&self, key: &str, default: Value) -> Value {
        self.get(key).ok().flatten().unwrap_or(default)
    }

    pub fn set(&self, key: &str, value: &Value) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO settings(key, value_json, updated_at)
             VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
             ON CONFLICT(key) DO UPDATE SET
               value_json = excluded.value_json,
               updated_at = excluded.updated_at",
            rusqlite::params![key, value.to_string()],
        )?;
        Ok(())
    }

    pub fn delete(&self, key: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let affected = conn.execute("DELETE FROM settings WHERE key = ?1", [key])?;
        Ok(affected > 0)
    }

    /// 只读访问底层连接（供其他模块复用同一线程池连接）。
    pub fn with_conn<T>(&self, f: impl FnOnce(&Connection) -> T) -> T {
        let conn = self.conn.lock().unwrap();
        f(&conn)
    }
}

#[tauri::command]
pub async fn get_setting(
    store: State<'_, std::sync::Arc<crate::AppState>>,
    key: String,
) -> Result<Option<Value>, String> {
    store.db.get(&key).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_setting(
    store: State<'_, std::sync::Arc<crate::AppState>>,
    key: String,
    value: Value,
) -> Result<(), String> {
    store.db.set(&key, &value).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_setting(
    store: State<'_, std::sync::Arc<crate::AppState>>,
    key: String,
) -> Result<bool, String> {
    store.db.delete(&key).map_err(|e| e.to_string())
}

/// 供 hotkey 模块持久化新快捷键到 settings 表。
pub fn persist_hotkey(app: &tauri::AppHandle, hotkey: &str) {
    use tauri::Manager;
    let state: tauri::State<std::sync::Arc<crate::AppState>> = app.state();
    let _ = state.db.set("ui.hotkey", &serde_json::json!(hotkey));
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn store() -> SettingsStore {
        SettingsStore::new(crate::db::open_in_memory().unwrap())
    }

    #[test]
    fn get_missing_returns_none() {
        assert_eq!(store().get("ui.density").unwrap(), None);
    }

    #[test]
    fn set_then_get_roundtrip() {
        let s = store();
        s.set("ui.density", &json!("compact")).unwrap();
        assert_eq!(s.get("ui.density").unwrap(), Some(json!("compact")));
    }

    #[test]
    fn set_overwrites_existing_value() {
        let s = store();
        s.set("ui.density", &json!("compact")).unwrap();
        s.set("ui.density", &json!("large")).unwrap();
        assert_eq!(s.get("ui.density").unwrap(), Some(json!("large")));
    }

    #[test]
    fn get_or_returns_default_when_missing() {
        let s = store();
        assert_eq!(s.get_or("x", json!(42)), json!(42));
    }

    #[test]
    fn delete_removes_key_and_reports_missing() {
        let s = store();
        s.set("ui.density", &json!("compact")).unwrap();
        assert!(s.delete("ui.density").unwrap());
        assert!(!s.delete("ui.density").unwrap());
        assert_eq!(s.get("ui.density").unwrap(), None);
    }
}
