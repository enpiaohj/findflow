use std::time::Instant;

use rusqlite::params;

use crate::models::{FileRow, QueryAst, SortMode};

const RANK_W: (f64, f64, f64, f64, f64, f64, f64) = (0.45, 0.20, 0.10, 0.10, 0.08, 0.05, 0.02);

pub struct SearchOutcome {
    pub items: Vec<FileRow>,
    pub total: i64,
    pub elapsed_ms: u128,
}

/// 执行一次搜索：Tantivy 提供候选 → SQLite 过滤补全 → 综合排序（设计文档 §9）。
pub fn run(
    store: &crate::settings::SettingsStore,
    index: &crate::searchindex::SearchIndex,
    ast: &QueryAst,
    limit: usize,
) -> Result<SearchOutcome, String> {
    let start = Instant::now();
    let mut candidate_scores: Option<Vec<(i64, f64)>> = None;

    let has_text = ast.text.is_some() || ast.phrase.is_some();
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
        let docs = index.candidate_docs(q.as_ref(), 600)?;
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

    // 先构建 SQL；文本候选存在时用临时表装载。
    let items: Vec<FileRow> = store.with_conn(|c| {
        if let Some(scored) = &candidate_scores {
            c.execute("DROP TABLE IF EXISTS temp.cands", []).ok();
            c.execute("CREATE TEMP TABLE cands(fid INTEGER PRIMARY KEY, sc REAL)", [])
                .map_err(|e| e.to_string())?;
            c.execute("BEGIN", []).map_err(|e| e.to_string())?;
            for (fid, sc) in scored.iter() {
                c.execute("INSERT OR REPLACE INTO cands VALUES(?1,?2)", params![fid, sc])
                    .map_err(|e| e.to_string())?;
            }
            c.execute("COMMIT", []).map_err(|e| e.to_string())?;
        }
        let mut where_parts: Vec<String> = vec!["1=1".into()];
        let mut bind: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        if let Some(scored) = &candidate_scores {
            where_parts.push(format!(
                "f.file_id IN (SELECT fid FROM temp.cands)"
            ));
            let _ = scored.len();
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
        for p in &ast.path_include {
            where_parts.push(format!("f.path LIKE ?"));
            bind.push(Box::new(format!("%{}%", p)));
        }
        for p in &ast.path_exclude {
            where_parts.push(format!("f.path NOT LIKE ?"));
            bind.push(Box::new(format!("%{}%", p)));
        }
        for t in &ast.text_exclude {
            where_parts.push(format!("f.name NOT LIKE ? AND f.ext <> ?"));
            bind.push(Box::new(format!("%{}%", t)));
            bind.push(Box::new(t.clone()));
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
        c.execute("DROP TABLE IF EXISTS temp.cands", []).ok();
        Ok::<Vec<FileRow>, String>(all)
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
                .prepare(
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

    let total: i64 = store.with_conn(|c| {
        // 估算总数（UI 显示“共 N 项”）：limit 上限时提示滚动加载即可。
        c.query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0))
            .map_err(|e| e.to_string())
    })?;
    let _ = &index;
    Ok(SearchOutcome {
        items,
        total,
        elapsed_ms: start.elapsed().as_millis(),
    })
}

fn recency_score(store: &crate::settings::SettingsStore, path: &str) -> f64 {
    let last: Option<String> = store.with_conn(|c| {
        c.query_row(
            "SELECT happened_at FROM recent_actions WHERE path=?1 ORDER BY id DESC LIMIT 1",
            [path],
            |r| r.get(0),
        )
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
pub fn cmd_search(
    store: tauri::State<'_, std::sync::Arc<crate::AppState>>,
    query: String,
    sort: Option<String>,
    limit: Option<usize>,
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
    let outcome = run(&store.db, &store.index, &ast, limit.unwrap_or(200))?;
    Ok(serde_json::json!({
        "items": outcome.items,
        "total": outcome.total,
        "elapsedMs": outcome.elapsed_ms,
        "ast": ast,
        "explain": ast.explain(),
    }))
}
