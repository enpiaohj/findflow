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
    "ALTER TABLE scopes ADD COLUMN files_est INTEGER;",
    // v3：音频 / 视频类型映射补全（新增 flac、ape、m2ts 等扩展名）。
    // kind 由扫描时写入，已有记录仍是旧映射；此处就地修正，避免用户重建索引。
    // 只更新 kind 列，值可由 ext 重新推导，无信息损失。
    "UPDATE files SET kind='audio' WHERE kind='other' AND ext IN
        ('mp3','wav','flac','aac','ogg','m4a','wma','ape','opus','aiff','aif','m4b','amr','mid','midi','ac3','dts');
     UPDATE files SET kind='video' WHERE kind='other' AND ext IN
        ('mp4','mkv','avi','mov','wmv','flv','webm','m4v','mpg','mpeg','rmvb','rm','m2ts','3gp','vob','asf','f4v');",
    // v4：(scope_id, parent_path) 复合索引。
    // 空搜索浏览（打开应用的默认视图）按「范围 + 父目录」过滤，原先只走 idx_files_scope
    // 命中整个范围（实测 92 万行）再逐行比对 parent_path 并建临时 B 树排序，约 260ms；
    // 复合索引后降至亚毫秒。索引体积约 1 个范围条目/行，构建耗时秒级。
    "CREATE INDEX IF NOT EXISTS idx_files_scope_parent ON files(scope_id, parent_path);",
    // v5：补全扩展名映射（Office 系列、图片、压缩包等）。
    // kind 由扫描时写入，已有记录仍是旧映射；此处就地修正，避免用户重建索引。
    // 只更新 kind 列，值可由 ext 重新推导，无信息损失。
    // 注意 `.ts` / `.mts` 有意不归视频（本机几乎全是 TypeScript），故不在列表中。
    "UPDATE files SET kind='doc' WHERE kind='other' AND ext IN
        ('docm','dot','dotx','odt','wps','pages','tex','epub','mht','mhtml');
     UPDATE files SET kind='sheet' WHERE kind='other' AND ext IN
        ('xlsm','xlsb','et','numbers','tsv');
     UPDATE files SET kind='image' WHERE kind='other' AND ext IN
        ('jfif','jpe','jp2','heic','heif','avif','psd','cr2','nef','arw','dng');
     UPDATE files SET kind='video' WHERE kind='other' AND ext IN
        ('m2v','3g2','divx','ogv','mxf');
     UPDATE files SET kind='audio' WHERE kind='other' AND ext IN
        ('oga','mka','caf','weba','ra');
     UPDATE files SET kind='archive' WHERE kind='other' AND ext IN
        ('tgz','txz','zst','lz4','lz','br','cab','iso','arj','lzh');",
    // v6：名称索引改为不分大小写。
    // 界面一律按 `name COLLATE NOCASE` 排序，而原 idx_files_name 是默认（区分大小写）排序规则，
    // 排序时根本用不上——按名称浏览要把全表读进临时 B 树（实测 85 万行约 80ms）。
    // 换成 NOCASE 后按名称排序可直接沿索引走（约 0.4ms），也是「边走边筛」快路径按名称排序的前提。
    // 一换一，索引数量与体积不变；构建约 1 秒 / 百万行。
    "CREATE INDEX IF NOT EXISTS idx_files_name_nocase ON files(name COLLATE NOCASE);
     DROP INDEX IF EXISTS idx_files_name;"
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

    #[test]
    fn migrate_v3_fixes_media_kinds_in_existing_rows() {
        // 构造一个停留在 v2 的库（v3 之前的状态），验证升级后旧记录被就地修正
        let conn = db_at_version(2);
        conn.execute(
            r"INSERT INTO scopes(scope_id, path, added_at) VALUES (1, 'D:\m', 'now')",
            [],
        )
        .unwrap();
        // 旧映射把音频/视频扩展名归到 other
        for (name, ext) in [("song.flac", "flac"), ("clip.m2ts", "m2ts"), ("note.txt", "txt")] {
            conn.execute(
                r"INSERT INTO files(scope_id, parent_path, name, path, ext, kind,
                    size, modified_ms, created_ms, is_dir, fulltext_state, updated_ms)
                  VALUES (1, 'D:\m', ?1, ?2, ?3, 'other', 1, 0, 0, 0, 'none', 0)",
                rusqlite::params![name, format!(r"D:\m\{name}"), ext],
            )
            .unwrap();
        }
        // 升级到最新
        migrate(&conn).unwrap();
        let kind_of = |name: &str| -> String {
            conn.query_row("SELECT kind FROM files WHERE name=?1", [name], |r| r.get(0))
                .unwrap()
        };
        assert_eq!(kind_of("song.flac"), "audio", "音频扩展名应被修正");
        assert_eq!(kind_of("clip.m2ts"), "video", "视频扩展名应被修正");
        assert_eq!(kind_of("note.txt"), "other", "无关扩展名不受影响");
    }

    /// 构造一个停留在指定版本的库，便于验证后续迁移。
    fn db_at_version(version: usize) -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        configure(&conn).unwrap();
        for (i, script) in MIGRATIONS.iter().take(version).enumerate() {
            conn.execute_batch(script).unwrap();
            conn.execute_batch(&format!("PRAGMA user_version = {};", i + 1))
                .unwrap();
        }
        conn
    }

    #[test]
    fn migrate_v5_completes_extension_kinds_in_existing_rows() {
        // v5 之前：Office / 图片 / 压缩包的补充格式都落在 other，筛选时会整类消失
        let conn = db_at_version(4);
        conn.execute(
            r"INSERT INTO scopes(scope_id, path, added_at) VALUES (1, 'D:\m', 'now')",
            [],
        )
        .unwrap();
        let cases = [
            ("a.xlsm", "xlsm", "sheet"),
            ("b.tsv", "tsv", "sheet"),
            ("c.odt", "odt", "doc"),
            ("d.wps", "wps", "doc"),
            ("e.mhtml", "mhtml", "doc"),
            ("f.heic", "heic", "image"),
            ("g.psd", "psd", "image"),
            ("h.cab", "cab", "archive"),
            ("i.tgz", "tgz", "archive"),
            ("j.oga", "oga", "audio"),
            ("k.ogv", "ogv", "video"),
            // 这两个必须保持 other：绝大多数是 TypeScript 源码 / 声明文件
            ("l.mts", "mts", "other"),
            ("m.ts", "ts", "other"),
        ];
        for (name, ext, _) in cases {
            conn.execute(
                r"INSERT INTO files(scope_id, parent_path, name, path, ext, kind,
                    size, modified_ms, created_ms, is_dir, fulltext_state, updated_ms)
                  VALUES (1, 'D:\m', ?1, ?2, ?3, 'other', 1, 0, 0, 0, 'none', 0)",
                rusqlite::params![name, format!(r"D:\m\{name}"), ext],
            )
            .unwrap();
        }
        migrate(&conn).unwrap();
        for (name, ext, want) in cases {
            let got: String = conn
                .query_row("SELECT kind FROM files WHERE name=?1", [name], |r| r.get(0))
                .unwrap();
            assert_eq!(got, want, "{ext} 的 kind 应为 {want}");
        }
    }

    #[test]
    fn migrate_v6_replaces_name_index_with_nocase() {
        let conn = open_in_memory().unwrap();
        let has = |name: &str| -> i64 {
            conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name=?1",
                [name],
                |r| r.get(0),
            )
            .unwrap()
        };
        assert_eq!(has("idx_files_name_nocase"), 1);
        assert_eq!(has("idx_files_name"), 0, "旧的区分大小写索引应被替换，不留两份");
        // 按名称（不分大小写）排序应直接走索引，不建临时 B 树
        let plan: Vec<String> = conn
            .prepare("EXPLAIN QUERY PLAN SELECT file_id FROM files ORDER BY name COLLATE NOCASE ASC, file_id ASC LIMIT 20")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(3))
            .unwrap()
            .map(|x| x.unwrap())
            .collect();
        let plan = plan.join(" / ");
        assert!(plan.contains("idx_files_name_nocase"), "{plan}");
        assert!(!plan.contains("TEMP B-TREE"), "{plan}");
    }

    #[test]
    fn migrate_creates_scope_parent_index() {
        let conn = open_in_memory().unwrap();
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_files_scope_parent'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1, "空搜索浏览依赖的复合索引必须存在");
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
