use std::path::Path;

use rusqlite::Connection;

// 迁移按序号执行，PRAGMA user_version 记录已应用版本；
// 已应用的脚本不得修改，只能追加新脚本。
const MIGRATIONS: &[&str] = &[
    // v1：设置表
    "CREATE TABLE settings (
        key TEXT PRIMARY KEY,
        value_json TEXT NOT NULL,
        updated_at TEXT NOT NULL
    );",
];

pub fn open(path: &Path) -> Result<Connection, rusqlite::Error> {
    let conn = Connection::open(path)?;
    configure(&conn)?;
    migrate(&conn)?;
    Ok(conn)
}

// 仅供单元测试使用的内存数据库。
#[cfg(test)]
pub fn open_in_memory() -> Result<Connection, rusqlite::Error> {
    let conn = Connection::open_in_memory()?;
    configure(&conn)?;
    migrate(&conn)?;
    Ok(conn)
}

fn configure(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.busy_timeout(std::time::Duration::from_secs(2))?;
    Ok(())
}

pub fn migrate(conn: &Connection) -> Result<(), rusqlite::Error> {
    let current: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    for (index, script) in MIGRATIONS.iter().enumerate() {
        let version = index as i64 + 1;
        if version > current {
            conn.execute_batch(script)?;
            // PRAGMA user_version 不能用绑定参数写回，pragma_update 会静默失效，
            // 必须用 execute_batch 拼接（version 来自内部计数，无注入风险）。
            conn.execute_batch(&format!("PRAGMA user_version = {version};"))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrate_creates_settings_table_and_records_version() {
        let conn = open_in_memory().unwrap();
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, MIGRATIONS.len() as i64);
        conn.execute(
            "INSERT INTO settings(key, value_json, updated_at) VALUES ('a', '\"b\"', 'now')",
            [],
        )
        .unwrap();
    }

    #[test]
    fn migrate_is_idempotent() {
        let conn = open_in_memory().unwrap();
        migrate(&conn).unwrap();
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, MIGRATIONS.len() as i64);
    }

    // 回归测试：pragma_update 写 user_version 在同一连接上可读但不会持久化到文件，
    // 必须重新打开文件数据库验证。
    #[test]
    fn migrate_persists_user_version_across_connections() {
        let path = std::env::temp_dir().join(format!("findflow-migrate-test-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);

        let conn = open(&path).unwrap();
        drop(conn);

        let conn = Connection::open(&path).unwrap();
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, MIGRATIONS.len() as i64);
        std::fs::remove_file(&path).ok();
    }
}
