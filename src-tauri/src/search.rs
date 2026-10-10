use std::time::Instant;

use rusqlite::params;

use crate::models::{FileRow, QueryAst, SortMode};

const RANK_W: (f64, f64, f64, f64, f64, f64, f64) = (0.45, 0.20, 0.10, 0.10, 0.08, 0.05, 0.02);

/// 全文候选上限。FTS 只能按相关度给出前 N 条，而类型 / 扩展名 / 大小 / 路径等过滤
/// 是在候选取出之后才由 SQL 完成的。若上限偏小，命中会因「先按相关度截断、再过滤」
/// 被白白丢掉——例如「报告 type:pdf」时 600 条候选里可能只有几条 pdf，
/// 而全库名含「报告」的 pdf 远不止这些。因此带过滤条件时放宽上限。
const CANDIDATES: usize = 600;
const CANDIDATES_WITH_FILTERS: usize = 4000;

pub struct SearchOutcome {
    pub items: Vec<FileRow>,
    /// 命中总数（有查询条件时为真实命中数，空搜索浏览时为索引总数）。
    /// 界面「共 N 项」用它，才能在只列出前 200 条时告诉用户实际有多少条。
    pub matched: i64,
    /// 索引总数（页脚「索引 N 条」）。
    pub total: i64,
    pub elapsed_ms: u128,
}

/// 执行一次搜索：Tantivy 提供候选 → SQLite 过滤补全 → 综合排序（设计文档 §9）。
/// `scope_id` 非 None 且 > 0 时仅浏览该索引范围；关键词搜索由前端始终传 None。
pub fn run(
    store: &crate::settings::SettingsStore,
    index: &crate::searchindex::SearchIndex,
    ast: &QueryAst,
    limit: usize,
    scope_id: Option<i64>,
) -> Result<SearchOutcome, String> {
    let start = Instant::now();
    let mut candidate_scores: Option<Vec<(i64, f64)>> = None;

    let has_text = ast.text.is_some() || ast.phrase.is_some();
    // 通配模式同样属于「有查询」：不应触发空搜索浏览的第一层限制
    let has_query = has_text || !ast.name_include.is_empty();
    let mut candidate_addrs: Option<Vec<(i64, tantivy::DocAddress)>> = None;
    let query_obj = if has_text {
        Some(index.build_query(
            ast.text.as_deref().unwrap_or(""),
            ast.phrase.as_deref(),
            true,
        ))
    } else {
        None
    };
    if has_text {
        let q = query_obj.as_ref().unwrap();
        let cap = if ast.has_filters() {
            CANDIDATES_WITH_FILTERS
        } else {
            CANDIDATES
        };
        let docs = index.candidate_docs(q.as_ref(), cap)?;
        // 候选顺序即相关度顺序，转换为可比较的分数。
        let scored: Vec<(i64, f64)> = docs
            .iter()
            .enumerate()
            .map(|(i, (id, _))| (*id as i64, docs.len() as f64 - i as f64))
            .collect();
        candidate_scores = Some(scored);
        candidate_addrs = Some(
            docs.into_iter()
                .map(|(id, addr)| (id as i64, addr))
                .collect(),
        );
    }

    // 有查询条件时，「共 N 项」必须是真实命中数。此前一律返回索引总数（实测 95 万），
    // 用户搜出 7 条却看到「共 952948 项」，会以为结果被截断而反复重搜。
    // 空搜索浏览没有任何条件，此时索引总数才等于命中数，继续用缓存避免每次按键全表 COUNT。
    let count_needed = has_query || ast.has_filters();

    // 先构建 SQL；文本候选存在时用临时表装载。
    let (items, matched_total): (Vec<FileRow>, Option<i64>) = store.with_conn(|c| {
        if let Some(scored) = &candidate_scores {
            c.execute("DROP TABLE IF EXISTS temp.cands", []).ok();
            c.execute("CREATE TEMP TABLE cands(fid INTEGER PRIMARY KEY, sc REAL)", [])
                .map_err(|e| e.to_string())?;
            c.execute("BEGIN", []).map_err(|e| e.to_string())?;
            {
                let mut ins = c
                    .prepare("INSERT OR REPLACE INTO cands VALUES(?1,?2)")
                    .map_err(|e| e.to_string())?;
                for (fid, sc) in scored.iter() {
                    ins.execute(params![fid, sc]).map_err(|e| e.to_string())?;
                }
            }
            c.execute("COMMIT", []).map_err(|e| e.to_string())?;
        }
        let mut where_parts: Vec<String> = vec!["1=1".into()];
        let mut bind: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        if let Some(scored) = &candidate_scores {
            where_parts.push("f.file_id IN (SELECT fid FROM temp.cands)".to_string());
            let _ = scored.len();
        }
        if let Some(sid) = scope_id {
            if sid > 0 {
                // 走 idx_files_scope 索引
                where_parts.push("f.scope_id = ?".into());
                bind.push(Box::new(sid));
                // 空搜索浏览为资源管理器式：只列出该索引位置根目录的第一层。
                // scope_id 仅由前端在无关键词时传入，关键词搜索不受影响
                if !has_query {
                    let root: Option<String> = c
                        .query_row(
                            "SELECT path FROM scopes WHERE scope_id = ?1",
                            [sid],
                            |r| r.get(0),
                        )
                        .ok();
                    if let Some(root) = root {
                        where_parts.push("f.parent_path = ?".into());
                        bind.push(Box::new(root));
                    }
                }
            }
        }
        if !ast.exts.is_empty() {
            let qs = vec!["?"; ast.exts.len()].join(",");
            where_parts.push(format!("f.ext IN ({qs})"));
            for e in &ast.exts {
                bind.push(Box::new(e.clone()));
            }
        }
        if !ast.kinds.is_empty() {
            let qs = vec!["?"; ast.kinds.len()].join(",");
            where_parts.push(format!("f.kind IN ({qs})"));
            for k in &ast.kinds {
                bind.push(Box::new(k.clone()));
            }
        }
        if !ast.kinds_exclude.is_empty() {
            let qs = vec!["?"; ast.kinds_exclude.len()].join(",");
            where_parts.push(format!("f.kind NOT IN ({qs})"));
            for k in &ast.kinds_exclude {
                bind.push(Box::new(k.clone()));
            }
        }
        for p in &ast.path_include {
            where_parts.push("f.path LIKE ?".to_string());
            bind.push(Box::new(format!("%{}%", p)));
        }
        for p in &ast.path_exclude {
            where_parts.push("f.path NOT LIKE ?".to_string());
            bind.push(Box::new(format!("%{}%", p)));
        }
        for t in &ast.text_exclude {
            where_parts.push("f.name NOT LIKE ? AND f.ext <> ?".to_string());
            bind.push(Box::new(format!("%{}%", t)));
            bind.push(Box::new(t.clone()));
        }
        // 文件名通配（`*` `?`）：按整名匹配，元字符已转义，故用 ESCAPE '\'
        for n in &ast.name_include {
            where_parts.push("f.name LIKE ? ESCAPE '\\'".to_string());
            bind.push(Box::new(crate::query::wildcard_to_like(n)));
        }
        for n in &ast.name_exclude {
            where_parts.push("f.name NOT LIKE ? ESCAPE '\\'".to_string());
            bind.push(Box::new(crate::query::wildcard_to_like(n)));
        }
        if let Some(s) = ast.size_min {
            where_parts.push("f.size >= ?".into());
            bind.push(Box::new(s));
        }
        if let Some(s) = ast.size_max {
            where_parts.push("f.size <= ?".into());
            bind.push(Box::new(s));
        }
        if let Some(m) = ast.modified_after_ms {
            where_parts.push("f.modified_ms >= ?".into());
            bind.push(Box::new(m));
        }
        if let Some(m) = ast.modified_before_ms {
            where_parts.push("f.modified_ms <= ?".into());
            bind.push(Box::new(m));
        }
        let order = match ast.sort {
            SortMode::Relevance => {
                if candidate_scores.is_some() {
                    "c.sc DESC, f.modified_ms DESC"
                } else if !ast.name_include.is_empty() {
                    // 纯通配查询：无文本候选，按名称序更接近通配匹配的直觉
                    "f.name COLLATE NOCASE ASC"
                } else if ast.modified_after_ms.is_some() || ast.modified_before_ms.is_some() {
                    // 最近筛选：按时间序走 modified 索引
                    "f.modified_ms DESC"
                } else {
                    // 纯筛选（如大文件）：按大小序走 (ext,size) 复合索引
                    "f.size DESC"
                }
            }
            SortMode::Name => "f.name COLLATE NOCASE ASC",
            SortMode::Modified => "f.modified_ms DESC",
            SortMode::Size => "f.size DESC",
        };
        let sql = format!(
            "SELECT f.file_id, f.name, f.path, f.ext, f.kind, f.size, f.modified_ms, f.created_ms, f.is_dir, f.fulltext_state,
                    CASE WHEN fav.file_id IS NULL THEN 0 ELSE 1 END
             FROM files f
             LEFT JOIN favorites fav ON fav.file_id = f.file_id
             {join}
             WHERE {where}
             ORDER BY {order}
             LIMIT {limit}",
            join = if candidate_scores.is_some() {
                "JOIN temp.cands c ON c.fid = f.file_id"
            } else {
                ""
            },
            where = where_parts.join(" AND "),
            limit = limit,
        );
        let mut stmt = c.prepare(&sql).map_err(|e| e.to_string())?;
        let refs: Vec<&dyn rusqlite::types::ToSql> = bind.iter().map(|b| b.as_ref()).collect();
        let rows = stmt
            .query_map(refs.as_slice(), |r| {
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
                    favorite: r.get::<_, i64>(10)? != 0,
                    tags: Vec::new(),
                })
            })
            .map_err(|e| e.to_string())?;
        let all: Vec<FileRow> = rows.filter_map(|r| r.ok()).collect();
        drop(stmt);
        // 必须在丢弃 temp.cands 之前统计：命中数依赖同一套 JOIN 与 WHERE
        let matched = if count_needed {
            let count_sql = format!(
                "SELECT COUNT(*) FROM files f {join} WHERE {where}",
                join = if candidate_scores.is_some() {
                    "JOIN temp.cands c ON c.fid = f.file_id"
                } else {
                    ""
                },
                where = where_parts.join(" AND "),
            );
            Some(
                c.query_row(&count_sql, refs.as_slice(), |r| r.get(0))
                    .map_err(|e| e.to_string())?,
            )
        } else {
            None
        };
        c.execute("DROP TABLE IF EXISTS temp.cands", []).ok();
        Ok::<(Vec<FileRow>, Option<i64>), String>((all, matched))
    })?;

    // 附加标签
    let mut items = items;
    // 命中片段：复用候选文档地址，零额外查询（FR06）
    if has_text {
        if let (Some(q), Some(addrs)) = (&query_obj, &candidate_addrs) {
            let addr_map: std::collections::HashMap<i64, tantivy::DocAddress> =
                addrs.iter().map(|(id, a)| (*id, *a)).collect();
            if let Some(gen) = index.snippet_generator(q.as_ref(), index.content) {
                let mut done_count = 0;
                for item in items.iter_mut() {
                    if done_count >= 30 {
                        break;
                    }
                    if item.fulltext_state != "done" {
                        continue;
                    }
                    if let Some(addr) = addr_map.get(&item.file_id) {
                        item.snippet = index.snippet_from_addr(&gen, *addr);
                        done_count += 1;
                    }
                }
            }
        }
    }

    store.with_conn(|c| {
        for item in &mut items {
            let mut stmt = c
                .prepare_cached(
                    "SELECT t.name FROM tags t JOIN file_tags ft ON ft.tag_id=t.tag_id WHERE ft.file_id=?1",
                )
                .map_err(|e| e.to_string())?;
            item.tags = stmt
                .query_map([item.file_id], |r| r.get::<_, String>(0))
                .map_err(|e| e.to_string())?
                .filter_map(|x| x.ok())
                .collect();
        }
        Ok::<_, String>(())
    })?;

    // 相关度综合排序（文本 45% / 完整命中 20% / 路径 10% / 最近打开 10% / 最近修改 8% / 收藏 5% / 历史 2%）
    if ast.sort == SortMode::Relevance && has_text {
        let max_sc = items.len() as f64;
        let text_of = ast.text.clone().unwrap_or_default();
        for (i, item) in items.iter_mut().enumerate() {
            let t_norm = 1.0 - (i as f64 / max_sc.max(1.0));
            let name_lower = item.name.to_lowercase();
            let q_lower = text_of.to_lowercase().replace(' ', "");
            let exact = if !q_lower.is_empty() && name_lower.contains(&q_lower) {
                1.0
            } else if text_of.split_whitespace().any(|w| name_lower.contains(&w.to_lowercase())) {
                0.5
            } else {
                0.0
            };
            let depth = 1.0 - (item.path.matches('\\').count() as f64 / 30.0).min(1.0);
            let open_rec = recency_score(store, &item.path);
            let mod_rec = recency_from_ms(item.modified_ms);
            let fav = if item.favorite { 1.0 } else { 0.0 };
            item.score = RANK_W.0 * t_norm
                + RANK_W.1 * exact
                + RANK_W.2 * depth
                + RANK_W.3 * open_rec
                + RANK_W.4 * mod_rec
                + RANK_W.5 * fav
                + RANK_W.6 * 0.0;
        }
        items.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    }

    let total = cached_total(store, scope_id)?;
    // 空搜索浏览没有查询条件，命中数即索引总数
    let matched = matched_total.unwrap_or(total);
    let _ = &index;
    Ok(SearchOutcome {
        items,
        matched,
        total,
        elapsed_ms: start.elapsed().as_millis(),
    })
}

/// 索引总数缓存条目：数据库身份、位置、写入时刻、计数。
type TotalCacheEntry = (usize, Option<i64>, Instant, i64);

/// 索引总数（UI 显示“共 N 项”）：百万级 COUNT(*) 需数十毫秒，每次按键都算会拖慢输入，
/// 缓存 10 秒即可满足显示需要。缓存键含数据库身份与位置：切换位置、切换数据库或超时才重算。
/// 指定位置时与空搜索浏览一致，只统计该位置根目录第一层。
fn cached_total(
    store: &crate::settings::SettingsStore,
    scope_id: Option<i64>,
) -> Result<i64, String> {
    static TOTAL: std::sync::Mutex<Option<TotalCacheEntry>> = std::sync::Mutex::new(None);
    // 缓存是进程级静态量，必须以数据库身份区分：否则不同库（如并行测试的内存库）
    // 会互相命中对方的计数。
    let db_id = store.id();
    let mut cache = TOTAL.lock().unwrap();
    if let Some((cached_db, cached_scope, at, n)) = *cache {
        if cached_db == db_id && cached_scope == scope_id && at.elapsed() < std::time::Duration::from_secs(10) {
            return Ok(n);
        }
    }
    let n: i64 = store.with_conn(|c| {
        if let Some(sid) = scope_id {
            // 与空搜索浏览一致：只统计该位置根目录第一层（位置不存在则计 0）
            let root: Option<String> = c
                .query_row("SELECT path FROM scopes WHERE scope_id = ?1", [sid], |r| r.get(0))
                .ok();
            match root {
                Some(root) => c.query_row(
                    "SELECT COUNT(*) FROM files WHERE scope_id = ?1 AND parent_path = ?2",
                    rusqlite::params![sid, root],
                    |r| r.get(0),
                ),
                None => Ok(0),
            }
        } else {
            c.query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0))
        }
        .map_err(|e| e.to_string())
    })?;
    *cache = Some((db_id, scope_id, Instant::now(), n));
    Ok(n)
}

fn recency_score(store: &crate::settings::SettingsStore, path: &str) -> f64 {
    let last: Option<String> = store.with_conn(|c| {
        c.prepare_cached(
            "SELECT happened_at FROM recent_actions WHERE path=?1 ORDER BY id DESC LIMIT 1",
        )
        .and_then(|mut s| s.query_row([path], |r| r.get(0)))
        .ok()
    });
    match last {
        Some(at) => {
            let t = chrono::DateTime::parse_from_rfc3339(&at)
                .map(|d| d.timestamp_millis())
                .unwrap_or(0);
            recency_from_ms(t)
        }
        None => 0.0,
    }
}

fn recency_from_ms(ms: i64) -> f64 {
    if ms <= 0 {
        return 0.0;
    }
    let age_days = (crate::util::now_ms() - ms) as f64 / 86_400_000.0;
    (1.0 - (age_days / 90.0)).clamp(0.0, 1.0)
}

/// 搜索命令：query 为筛选语法文本；nl=true 时先走离线自然语言解析。
#[tauri::command]
pub async fn cmd_search(
    store: tauri::State<'_, std::sync::Arc<crate::AppState>>,
    query: String,
    sort: Option<String>,
    limit: Option<usize>,
    scope_id: Option<i64>,
) -> Result<serde_json::Value, String> {
    let mut ast = if query.trim().is_empty() {
        crate::models::QueryAst::default()
    } else {
        crate::query::parse(&query)
    };
    if let Some(s) = &sort {
        ast.sort = match s.as_str() {
            "name" => crate::models::SortMode::Name,
            "modified" => crate::models::SortMode::Modified,
            "size" => crate::models::SortMode::Size,
            _ => crate::models::SortMode::Relevance,
        };
    }
    let outcome = run(&store.db, &store.index, &ast, limit.unwrap_or(200), scope_id)?;
    Ok(serde_json::json!({
        "items": outcome.items,
        "matched": outcome.matched,
        "total": outcome.total,
        "elapsedMs": outcome.elapsed_ms,
        "ast": ast,
        "explain": ast.explain(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 空搜索浏览：scope_id 限定范围且仅列根目录第一层，名称与大小排序保持稳定。
    #[test]
    fn scope_filter_and_browse_sorts() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::settings::SettingsStore::new(crate::db::open_in_memory().unwrap());
        let index = crate::searchindex::SearchIndex::open_or_create(dir.path()).unwrap();
        store.with_conn(|c| {
            c.execute(
                "INSERT INTO scopes(scope_id, path, added_at) VALUES (1, 'D:/a', 'now'), (2, 'D:/b', 'now')",
                [],
            )
            .unwrap();
            for (sid, name, size, parent) in [
                (1, "zeta.txt", 30_i64, "D:/a"),
                (1, "alpha.txt", 10_i64, "D:/a"),
                (2, "beta.txt", 20_i64, "D:/b"),
                // 子目录内的文件：属于该范围，但不属于根目录第一层
                (1, "deep.txt", 40_i64, "D:/a/sub"),
            ] {
                c.execute(
                    "INSERT INTO files(scope_id, name, path, kind, size, parent_path)
                     VALUES (?1, ?2, ?3, 'doc', ?4, ?5)",
                    rusqlite::params![sid, name, format!("{parent}\\{name}"), size, parent],
                )
                .unwrap();
            }
        });

        let by_size = QueryAst {
            sort: SortMode::Size,
            ..Default::default()
        };
        let all = run(&store, &index, &by_size, 10, None).unwrap();
        assert_eq!(
            all.items
                .iter()
                .map(|item| item.name.as_str())
                .collect::<Vec<_>>(),
            vec!["deep.txt", "zeta.txt", "beta.txt", "alpha.txt"]
        );
        assert_eq!(all.total, 4);

        let by_name = QueryAst {
            sort: SortMode::Name,
            ..Default::default()
        };
        // 指定位置：只返回该位置根目录第一层的条目，子目录内的 deep.txt 不出现
        let scoped = run(&store, &index, &by_name, 10, Some(1)).unwrap();
        assert_eq!(
            scoped
                .items
                .iter()
                .map(|item| item.name.as_str())
                .collect::<Vec<_>>(),
            vec!["alpha.txt", "zeta.txt"]
        );
        assert_eq!(scoped.total, 2);

        // 不存在的位置：结果为空
        let unknown_scope = run(&store, &index, &by_name, 10, Some(99)).unwrap();
        assert!(unknown_scope.items.is_empty());
        assert_eq!(unknown_scope.total, 0);
    }

    /// 「共 N 项」必须是真实命中数：结果被 200 条上限截断时，
    /// 界面要能告诉用户实际命中了多少条，而不是只报列出的条数。
    #[test]
    fn matched_counts_all_hits_not_just_returned_page() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::settings::SettingsStore::new(crate::db::open_in_memory().unwrap());
        let index = crate::searchindex::SearchIndex::open_or_create(dir.path()).unwrap();
        store.with_conn(|c| {
            c.execute(
                "INSERT INTO scopes(scope_id, path, added_at) VALUES (1, 'D:/a', 'now')",
                [],
            )
            .unwrap();
        });
        for i in 0..5 {
            let name = format!("会议纪要{i}.txt");
            let path = format!("D:/a/{name}");
            let fid = store.with_conn(|c| {
                c.execute(
                    "INSERT INTO files(scope_id, name, path, kind, size, parent_path)
                     VALUES (1, ?1, ?2, 'doc', 1, 'D:/a')",
                    rusqlite::params![name, path],
                )
                .unwrap();
                c.last_insert_rowid() as u64
            });
            index.add(index.build_doc(fid, &name, None, None)).unwrap();
        }
        index.commit().unwrap();
        index.reload();

        let ast = QueryAst {
            text: Some("会议".into()),
            ..Default::default()
        };
        let out = run(&store, &index, &ast, 2, None).unwrap();
        assert_eq!(out.items.len(), 2, "只返回上限条数");
        assert_eq!(out.matched, 5, "命中数应为全部 5 条，而不是返回的 2 条");

        // 空搜索浏览没有条件，命中数即索引总数
        let browse = QueryAst::default();
        let out2 = run(&store, &index, &browse, 2, None).unwrap();
        assert_eq!(out2.matched, 5);
        assert_eq!(out2.total, 5);
    }

    /// 通配模式按整名匹配：`*` 任意多字符、`?` 单字符，且元字符被转义。
    #[test]
    fn wildcard_filters_by_full_name() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::settings::SettingsStore::new(crate::db::open_in_memory().unwrap());
        let index = crate::searchindex::SearchIndex::open_or_create(dir.path()).unwrap();
        store.with_conn(|c| {
            c.execute(
                "INSERT INTO scopes(scope_id, path, added_at) VALUES (1, 'D:/a', 'now')",
                [],
            )
            .unwrap();
            for name in [
                "报告-2026.pdf",
                "报告-2027.pdf",
                "预算.xlsx",
                "100%_增长.txt",
                "note-01.txt",
                "note-02.txt",
                "note-1.txt",
            ] {
                c.execute(
                    "INSERT INTO files(scope_id, name, path, kind, size, parent_path)
                     VALUES (1, ?1, ?2, 'doc', 1, 'D:/a')",
                    rusqlite::params![name, format!("D:/a\\{name}")],
                )
                .unwrap();
            }
        });

        let names = |ast: &QueryAst| -> Vec<String> {
            let mut v: Vec<String> = run(&store, &index, ast, 20, None)
                .unwrap()
                .items
                .iter()
                .map(|i| i.name.clone())
                .collect();
            v.sort();
            v
        };
        let wc = |patterns: &[&str]| QueryAst {
            name_include: patterns.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        };

        // 前缀 + 任意扩展名
        assert_eq!(names(&wc(&["报告*"])), vec!["报告-2026.pdf", "报告-2027.pdf"]);
        // 任意名 + 指定扩展名
        assert_eq!(names(&wc(&["*.pdf"])), vec!["报告-2026.pdf", "报告-2027.pdf"]);
        // ? 只匹配单个字符：note-0?.txt 不含 note-1.txt
        assert_eq!(names(&wc(&["note-0?.txt"])), vec!["note-01.txt", "note-02.txt"]);
        // 用户输入的 % 是字面量，不是通配
        assert_eq!(names(&wc(&["100%_增长.txt"])), vec!["100%_增长.txt"]);
        assert!(names(&wc(&["100%增长*"])).is_empty(), "% 不应匹配任意字符");

        // 排除模式
        let excluded = QueryAst {
            name_include: vec!["*.pdf".into()],
            name_exclude: vec!["*2027*".into()],
            ..Default::default()
        };
        assert_eq!(names(&excluded), vec!["报告-2026.pdf"]);

        // 纯通配查询默认按名称排序，而非落到「按大小」
        let plain = run(&store, &index, &wc(&["note-*"]), 20, None).unwrap();
        assert_eq!(
            plain.items.iter().map(|i| i.name.as_str()).collect::<Vec<_>>(),
            vec!["note-01.txt", "note-02.txt", "note-1.txt"]
        );
    }

    /// 通配查询不应被当作空搜索浏览（不受 scope_id 的第一层限制）。
    #[test]
    fn wildcard_is_not_treated_as_empty_browse() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::settings::SettingsStore::new(crate::db::open_in_memory().unwrap());
        let index = crate::searchindex::SearchIndex::open_or_create(dir.path()).unwrap();
        store.with_conn(|c| {
            c.execute(
                "INSERT INTO scopes(scope_id, path, added_at) VALUES (1, 'D:/a', 'now')",
                [],
            )
            .unwrap();
            c.execute(
                "INSERT INTO files(scope_id, name, path, kind, size, parent_path)
                 VALUES (1, 'deep.pdf', 'D:/a/sub\\deep.pdf', 'doc', 1, 'D:/a/sub')",
                [],
            )
            .unwrap();
        });
        let ast = QueryAst {
            name_include: vec!["*.pdf".into()],
            ..Default::default()
        };
        // 即便传了 scope_id，通配查询也应覆盖子目录
        let out = run(&store, &index, &ast, 10, Some(1)).unwrap();
        assert_eq!(out.items.len(), 1, "子目录内的通配命中不应被第一层限制滤掉");
    }
}
