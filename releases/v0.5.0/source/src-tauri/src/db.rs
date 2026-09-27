use rusqlite::Connection;
use std::path::Path;

// 迁移按序号执行，PRAGMA user_version 记录已应用版本；
// 已应用的脚本不得修改，只能追加新脚本。
const MIGRATIONS: &[&str] = &[
    // v1：全量基线 Schema（设计文档 §8.1）
    "
    CREATE TABLE settings (
        key TEXT PRIMARY KEY,
        value_json TEXT NOT NULL,
        updated_at TEXT NOT NULL
    );

    CREATE TABLE volumes (
        volume_id INTEGER PRIMARY KEY,
        root_path TEXT NOT NULL,
        serial INTEGER NOT NULL,
        fs_type TEXT NOT NULL,
        state TEXT NOT NULL DEFAULT 'ok',
        journal_id TEXT,
        last_usn INTEGER,
        updated_ms INTEGER NOT NULL
    );
    CREATE TABLE scopes (
        scope_id INTEGER PRIMARY KEY,
        path TEXT NOT NULL UNIQUE,
        enabled INTEGER NOT NULL DEFAULT 1,
        options_json TEXT NOT NULL DEFAULT '{}',
        state TEXT NOT NULL DEFAULT 'idle',
        added_at TEXT NOT NULL,
        baseline_at TEXT,
        last_scan_ms INTEGER
    );
    CREATE TABLE files (
        file_id INTEGER PRIMARY KEY,
        scope_id INTEGER NOT NULL REFERENCES scopes(scope_id) ON DELETE CASCADE,
        native_serial INTEGER,
        native_file_id INTEGER,
        parent_path TEXT,
        name TEXT NOT NULL,
        path TEXT NOT NULL UNIQUE,
        ext TEXT NOT NULL DEFAULT '',
        kind TEXT NOT NULL,
        size INTEGER NOT NULL DEFAULT 0,
        created_ms INTEGER NOT NULL DEFAULT 0,
        modified_ms INTEGER NOT NULL DEFAULT 0,
        attributes INTEGER NOT NULL DEFAULT 0,
        is_dir INTEGER NOT NULL DEFAULT 0,
        fulltext_state TEXT NOT NULL DEFAULT 'none',
        ocr_state TEXT NOT NULL DEFAULT 'none',
        updated_ms INTEGER NOT NULL DEFAULT 0
    );
    CREATE INDEX idx_files_name ON files(name);
    CREATE INDEX idx_files_kind ON files(kind);
    CREATE INDEX idx_files_size ON files(size);
    CREATE INDEX idx_files_modified ON files(modified_ms);
    CREATE INDEX idx_files_scope ON files(scope_id);
    CREATE INDEX idx_files_ext ON files(ext);
    CREATE INDEX idx_files_ext_size ON files(ext, size);
    CREATE TABLE content_state (
        file_id INTEGER PRIMARY KEY REFERENCES files(file_id) ON DELETE CASCADE,
        extractor TEXT NOT NULL,
        content_hash TEXT,
        status TEXT NOT NULL,
        error_code TEXT,
        indexed_ms INTEGER NOT NULL DEFAULT 0
    );
    CREATE TABLE content_text (
        file_id INTEGER PRIMARY KEY REFERENCES files(file_id) ON DELETE CASCADE,
        content_text TEXT NOT NULL
    );
    CREATE TABLE ocr_text (
        file_id INTEGER PRIMARY KEY REFERENCES files(file_id) ON DELETE CASCADE,
        ocr_text TEXT NOT NULL
    );
    CREATE TABLE favorites (
        file_id INTEGER PRIMARY KEY REFERENCES files(file_id) ON DELETE CASCADE,
        fallback_path TEXT NOT NULL,
        created_at TEXT NOT NULL
    );
    CREATE TABLE tags (
        tag_id INTEGER PRIMARY KEY,
        name TEXT NOT NULL UNIQUE,
        color TEXT NOT NULL
    );
    CREATE TABLE file_tags (
        file_id INTEGER NOT NULL REFERENCES files(file_id) ON DELETE CASCADE,
        tag_id INTEGER NOT NULL REFERENCES tags(tag_id) ON DELETE CASCADE,
        PRIMARY KEY (file_id, tag_id)
    );
    CREATE TABLE recent_actions (
        id INTEGER PRIMARY KEY,
        file_id INTEGER,
        path TEXT NOT NULL,
        action TEXT NOT NULL,
        happened_at TEXT NOT NULL
    );
    CREATE INDEX idx_recent_path ON recent_actions(path);
    CREATE TABLE search_history (
        id INTEGER PRIMARY KEY,
        query TEXT NOT NULL,
        at TEXT NOT NULL
    );
    CREATE TABLE jobs (
        job_id INTEGER PRIMARY KEY,
        kind TEXT NOT NULL,
        state TEXT NOT NULL,
        progress REAL NOT NULL DEFAULT 0,
        message TEXT,
        checkpoint_json TEXT,
        result_json TEXT,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
    );
    CREATE TABLE operation_log (
        operation_id INTEGER PRIMARY KEY,
        kind TEXT NOT NULL,
        request_json TEXT NOT NULL,
        result_json TEXT NOT NULL,
        reversible_until TEXT,
        created_at TEXT NOT NULL
    );
    CREATE TABLE rename_undo (
        operation_id INTEGER NOT NULL,
        file_id INTEGER NOT NULL,
        old_path TEXT NOT NULL,
        new_path TEXT NOT NULL,
        PRIMARY KEY (operation_id, file_id)
    );
    CREATE TABLE index_outbox (
        event_id INTEGER PRIMARY KEY,
        file_id INTEGER NOT NULL,
        operation TEXT NOT NULL,
        state TEXT NOT NULL DEFAULT 'pending',
        created_ms INTEGER NOT NULL
    );
    CREATE INDEX idx_outbox_pending ON index_outbox(state);
    ",
    // v2：范围预估文件数（试扫确认值，用于扫描进度百分比）
    "ALTER TABLE scopes ADD COLUMN files_est INTEGER;"
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
    // 读性能：页缓存 8MB + mmap 256MB（WAL 模式下安全）
    conn.pragma_update(None, "cache_size", -8000)?;
    conn.pragma_update(None, "mmap_size", 268_435_456)?;
    // 检查点完成后把 WAL 文件截回 64MB 以内，避免长期运行后 WAL 无限膨胀
    conn.pragma_update(None, "journal_size_limit", 67_108_864)?;
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
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");
        let conn = open(&path).unwrap();
        drop(conn);
        let conn = Connection::open(&path).unwrap();
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, MIGRATIONS.len() as i64);
    }

    #[test]
    fn schema_supports_core_flows() {
        let conn = open_in_memory().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO scopes(path, added_at) VALUES ('D:\\\\demo', ?1)",
            [&now],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO files(scope_id, name, path, kind) VALUES (1, 'a.txt', 'D:\\\\demo\\\\a.txt', 'doc')",
            [],
        )
        .unwrap();
        let fid: i64 = conn.query_row("SELECT file_id FROM files", [], |r| r.get(0)).unwrap();
        conn.execute(
            "INSERT INTO favorites(file_id, fallback_path, created_at) VALUES (?1, 'p', ?2)",
            rusqlite::params![fid, now],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO index_outbox(file_id, operation, created_ms) VALUES (?1, 'upsert', 0)",
            [fid],
        )
        .unwrap();
    }
}
