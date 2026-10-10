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
    /// 命中数统计超出时间预算、尚未算完：此时 `matched` 只是已列出的条数（下限），
    /// 精确值由前端随后调用 `cmd_search_count` 取得。
    pub matched_pending: bool,
}

/// 列表与计数的执行策略。
#[derive(Clone, Copy, Debug)]
pub struct Tuning {
    /// 纯筛选查询（无关键词候选）先尝试「沿排序索引边走边筛」；超过该预算（毫秒）即放弃，退回常规计划。
    /// `None` 表示不尝试。
    pub scan_budget_ms: Option<u64>,
    /// 命中数统计的时间预算（毫秒）；超时返回 `matched_pending`。`None` 表示不限时、必得精确值。
    pub count_budget_ms: Option<u64>,
}

impl Tuning {
    /// 精确模式：不走快路径、计数不限时。测试与后台计数使用，结果确定。
    pub const EXACT: Tuning = Tuning { scan_budget_ms: None, count_budget_ms: None };
    /// 交互模式：界面搜索使用。两个预算合计是最坏情况下多付出的等待（约 45ms）。
    pub const INTERACTIVE: Tuning = Tuning { scan_budget_ms: Some(25), count_budget_ms: Some(20) };
}

const ROW_SELECT: &str = "SELECT f.file_id, f.name, f.path, f.ext, f.kind, f.size, f.modified_ms, f.created_ms, f.is_dir, f.fulltext_state,
                    CASE WHEN fav.file_id IS NULL THEN 0 ELSE 1 END
             FROM files f
             LEFT JOIN favorites fav ON fav.file_id = f.file_id";

/// 执行列表查询并映射成行。行级错误必须上抛：查询被时间预算中断时错误出现在取行阶段，
/// 若像以前那样丢弃出错的行，会把「只走了一半」的结果当成完整结果返回。
fn fetch_rows(
    c: &rusqlite::Connection,
    sql: &str,
    refs: &[&dyn rusqlite::types::ToSql],
) -> rusqlite::Result<Vec<FileRow>> {
    let mut stmt = c.prepare(sql)?;
    let rows = stmt.query_map(refs, |r| {
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
    })?;
    rows.collect()
}

/// 在时间预算内执行 `f`；超时（SQLite 进度回调中断）返回 `Ok(None)`，其它错误照常上抛。
/// 预算为 0 时每条虚拟机指令都检查一次，用于测试中确定性地触发「超时」。
fn with_budget<T>(
    c: &rusqlite::Connection,
    budget_ms: u64,
    f: impl FnOnce() -> rusqlite::Result<T>,
) -> Result<Option<T>, String> {
    let deadline = Instant::now() + std::time::Duration::from_millis(budget_ms);
    let every = if budget_ms == 0 { 1 } else { 1000 };
    c.progress_handler(every, Some(move || Instant::now() >= deadline));
    let result = f();
    c.progress_handler(0, None::<fn() -> bool>);
    match result {
        Ok(v) => Ok(Some(v)),
        Err(rusqlite::Error::SqliteFailure(e, _)) if e.code == rusqlite::ErrorCode::OperationInterrupted => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

/// 给筛选条件里的列加一元 `+`，让 SQLite 不再用这些列上的索引去「先筛出全部匹配行」，
/// 从而沿排序列的索引顺序扫描、边走边筛。排序列自身的条件保留可用索引（范围条件正好缩小扫描区间）。
fn deindex(part: &str, keep: &str) -> String {
    const COLS: [&str; 8] = [
        "f.scope_id", "f.parent_path", "f.ext", "f.kind", "f.size", "f.modified_ms", "f.name", "f.path",
    ];
    let mut out = part.to_string();
    for col in COLS {
        if col != keep {
            out = out.replace(col, &format!("+{col}"));
        }
    }
    out
}

/// 最近超时过的快路径查询：60 秒内不再尝试，避免稀疏条件下每次按键都白付一次预算。
static SCAN_MISS: std::sync::Mutex<Vec<(String, Instant)>> = std::sync::Mutex::new(Vec::new());
const SCAN_MISS_TTL: std::time::Duration = std::time::Duration::from_secs(60);

fn scan_recently_missed(key: &str) -> bool {
    let mut misses = SCAN_MISS.lock().unwrap();
    misses.retain(|(_, at)| at.elapsed() < SCAN_MISS_TTL);
    misses.iter().any(|(k, _)| k == key)
}

fn remember_scan_miss(key: String) {
    let mut misses = SCAN_MISS.lock().unwrap();
    if misses.len() >= 64 {
        misses.remove(0);
    }
    misses.push((key, Instant::now()));
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
    run_opts(store, index, ast, limit, scope_id, Tuning::EXACT)
}

/// 同 [`run`]，可指定执行策略（界面搜索用 [`Tuning::INTERACTIVE`]）。两种策略返回的条目与顺序一致。
pub fn run_opts(
    store: &crate::settings::SettingsStore,
    index: &crate::searchindex::SearchIndex,
    ast: &QueryAst,
    limit: usize,
    scope_id: Option<i64>,
    tuning: Tuning,
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
    let (items, matched_total, matched_pending): (Vec<FileRow>, Option<i64>, bool) = store.with_conn(|c| {
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
        // 空搜索浏览「只列根目录第一层」：条件高度稀疏且已有专用复合索引，不走边走边筛
        let mut first_layer = false;
        if let Some(scored) = &candidate_scores {
            where_parts.push("f.file_id IN (SELECT fid FROM temp.cands)".to_string());
            let _ = scored.len();
        }
        // 已关闭的索引范围不参与搜索（开关只控制监听与扫描是不够的，用户期望关闭即不再命中）
        if has_disabled_scope(c) {
            where_parts.push("f.scope_id IN (SELECT scope_id FROM scopes WHERE enabled=1)".into());
        }
        if let Some(sid) = scope_id {
            if sid > 0 {
                // 走 idx_files_scope 索引
                where_parts.push("f.scope_id = ?".into());
                bind.push(Box::new(sid));
                // 空搜索浏览为资源管理器式：只列出该索引位置根目录的第一层。
                // scope_id 仅由前端在无关键词时传入，关键词搜索不受影响
                // 带类型 / 时间等筛选时是「按条件找文件」，不是浏览，同样不受第一层限制
                if !has_query && !ast.has_filters() {
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
                        first_layer = true;
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
            if let Some((dir, like)) = crate::query::path_scope(p) {
                // 绝对路径：该目录本身及其下全部内容（按目录边界，不误中同前缀的兄弟目录）
                where_parts.push("(f.path = ? COLLATE NOCASE OR f.path LIKE ? ESCAPE '\\')".to_string());
                bind.push(Box::new(dir));
                bind.push(Box::new(like));
            } else {
                where_parts.push("f.path LIKE ? ESCAPE '\\'".to_string());
                bind.push(Box::new(crate::query::contains_like(p)));
            }
        }
        for p in &ast.path_exclude {
            if let Some((dir, like)) = crate::query::path_scope(p) {
                where_parts.push("NOT (f.path = ? COLLATE NOCASE OR f.path LIKE ? ESCAPE '\\')".to_string());
                bind.push(Box::new(dir));
                bind.push(Box::new(like));
            } else {
                where_parts.push("f.path NOT LIKE ? ESCAPE '\\'".to_string());
                bind.push(Box::new(crate::query::contains_like(p)));
            }
        }
        for t in &ast.text_exclude {
            where_parts.push("f.name NOT LIKE ? ESCAPE '\\' AND f.ext <> ?".to_string());
            bind.push(Box::new(crate::query::contains_like(t)));
            bind.push(Box::new(t.clone()));
        }
        // 文件名通配（`*` `?`）：按整名匹配，元字符已转义，故用 ESCAPE '\'
        for n in &ast.name_include {
            if let Some(ext) = crate::query::wildcard_ext_only(n) {
                // `*.pdf`：扩展名走索引，比对整库做 LIKE 快得多（文件夹的 ext 为空，不会误命中）
                where_parts.push("f.ext = ?".to_string());
                bind.push(Box::new(ext));
            } else {
                where_parts.push("f.name LIKE ? ESCAPE '\\'".to_string());
                bind.push(Box::new(crate::query::wildcard_to_like(n)));
            }
        }
        for n in &ast.name_exclude {
            if let Some(ext) = crate::query::wildcard_ext_only(n) {
                where_parts.push("f.ext <> ?".to_string());
                bind.push(Box::new(ext));
            } else {
                where_parts.push("f.name NOT LIKE ? ESCAPE '\\'".to_string());
                bind.push(Box::new(crate::query::wildcard_to_like(n)));
            }
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
        // 排序键后追加同方向的 file_id：并列时次序确定，且仍可直接沿该列索引扫描（索引末尾隐含行号）。
        // `sort_col` 为 None 表示按候选相关度排序（有关键词），不适用边走边筛。
        let (order, sort_col): (&str, Option<&str>) = {
            const NAME: (&str, Option<&str>) = ("f.name COLLATE NOCASE ASC, f.file_id ASC", Some("f.name"));
            const MODIFIED: (&str, Option<&str>) = ("f.modified_ms DESC, f.file_id DESC", Some("f.modified_ms"));
            const SIZE: (&str, Option<&str>) = ("f.size DESC, f.file_id DESC", Some("f.size"));
            match ast.sort {
                SortMode::Relevance => {
                    if candidate_scores.is_some() {
                        ("c.sc DESC, f.modified_ms DESC", None)
                    } else if !ast.name_include.is_empty() {
                        // 纯通配查询：无文本候选，按名称序更接近通配匹配的直觉
                        NAME
                    } else if ast.modified_after_ms.is_some() || ast.modified_before_ms.is_some() {
                        // 最近筛选：按时间序
                        MODIFIED
                    } else {
                        // 纯筛选（如大文件）：按大小序
                        SIZE
                    }
                }
                SortMode::Name => NAME,
                SortMode::Modified => MODIFIED,
                SortMode::Size => SIZE,
            }
        };
        let join = if candidate_scores.is_some() {
            "JOIN temp.cands c ON c.fid = f.file_id"
        } else {
            ""
        };
        let where_sql = where_parts.join(" AND ");
        let refs: Vec<&dyn rusqlite::types::ToSql> = bind.iter().map(|b| b.as_ref()).collect();

        // 快路径：沿排序索引边走边筛，凑够 limit 条就停。
        // 常规计划是「先用筛选列索引找出全部匹配行再排序」，匹配行多时（点「文档」「图片」按钮）
        // 要读十几万到几十万行；而匹配行越多，沿排序索引凑够 200 条越快（实测 85 万行：
        // 文档 371ms → 1.8ms，图片 123ms → 3.7ms）。匹配行稀疏时要走很远，故设时间预算，
        // 超时即放弃并退回常规计划——那时匹配行少，常规计划本来就快。
        let mut scanned: Option<Vec<FileRow>> = None;
        if let (Some(budget), Some(col), None, false) =
            (tuning.scan_budget_ms, sort_col, &candidate_scores, first_layer)
        {
            if limit > 0 && where_parts.len() > 1 {
                let key = format!(
                    "{}|{:?}|{}|{}",
                    store.id(),
                    scope_id,
                    order,
                    serde_json::to_string(ast).unwrap_or_default()
                );
                if !scan_recently_missed(&key) {
                    let scan_where = where_parts
                        .iter()
                        .map(|part| deindex(part, col))
                        .collect::<Vec<_>>()
                        .join(" AND ");
                    let sql = format!("{ROW_SELECT} WHERE {scan_where} ORDER BY {order} LIMIT {limit}");
                    match with_budget(c, budget, || fetch_rows(c, &sql, refs.as_slice()))? {
                        Some(rows) => scanned = Some(rows),
                        None => remember_scan_miss(key),
                    }
                }
            }
        }
        // 快路径没凑够 limit 条就结束，说明把全部匹配行都走完了：条数即精确命中数，无需再统计
        let scan_complete = scanned.as_ref().is_some_and(|rows| rows.len() < limit);
        let all = match scanned {
            Some(rows) => rows,
            None => {
                let sql = format!("{ROW_SELECT} {join} WHERE {where_sql} ORDER BY {order} LIMIT {limit}");
                fetch_rows(c, &sql, refs.as_slice()).map_err(|e| e.to_string())?
            }
        };

        // 必须在丢弃 temp.cands 之前统计：命中数依赖同一套 JOIN 与 WHERE
        let mut pending = false;
        let matched = if !count_needed {
            None
        } else if scan_complete {
            Some(all.len() as i64)
        } else {
            let count_sql = format!("SELECT COUNT(*) FROM files f {join} WHERE {where_sql}");
            let count = || c.query_row(&count_sql, refs.as_slice(), |r| r.get::<_, i64>(0));
            match (tuning.count_budget_ms, candidate_scores.is_none()) {
                // 纯筛选的精确计数可能要逐行检查十几万行（如「图片 + 最近 7 天」约 120ms），
                // 不值得让列表等它：限时统计，超时先报已列出的条数，精确值由前端随后补取
                (Some(budget), true) => match with_budget(c, budget, count)? {
                    Some(n) => Some(n),
                    None => {
                        pending = true;
                        Some(all.len() as i64)
                    }
                },
                _ => Some(count().map_err(|e| e.to_string())?),
            }
        };
        c.execute("DROP TABLE IF EXISTS temp.cands", []).ok();
        Ok::<(Vec<FileRow>, Option<i64>, bool), String>((all, matched, pending))
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
        matched_pending,
    })
}

/// 索引总数缓存条目：数据库身份、位置、写入时刻、计数。
type TotalCacheEntry = (usize, Option<i64>, Instant, i64);

static TOTAL: std::sync::Mutex<Option<TotalCacheEntry>> = std::sync::Mutex::new(None);

/// 索引总数（UI 显示“共 N 项”）：百万级 COUNT(*) 需数十毫秒，每次按键都算会拖慢输入，
/// 缓存 10 秒即可满足显示需要。缓存键含数据库身份与位置：切换位置、切换数据库或超时才重算。
/// 指定位置时与空搜索浏览一致，只统计该位置根目录第一层。
fn cached_total(
    store: &crate::settings::SettingsStore,
    scope_id: Option<i64>,
) -> Result<i64, String> {
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
        } else if has_disabled_scope(c) {
            c.query_row(
                "SELECT COUNT(*) FROM files WHERE scope_id IN (SELECT scope_id FROM scopes WHERE enabled=1)",
                [],
                |r| r.get(0),
            )
        } else {
            c.query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0))
        }
        .map_err(|e| e.to_string())
    })?;
    *cache = Some((db_id, scope_id, Instant::now(), n));
    Ok(n)
}

/// 是否存在已关闭的索引范围。
fn has_disabled_scope(c: &rusqlite::Connection) -> bool {
    c.query_row("SELECT EXISTS(SELECT 1 FROM scopes WHERE enabled=0)", [], |r| r.get::<_, i64>(0))
        .map(|v| v != 0)
        .unwrap_or(false)
}

/// 索引范围开关 / 增删后让总数缓存失效，否则最多 10 秒内仍显示旧数字。
pub fn invalidate_total_cache() {
    *TOTAL.lock().unwrap() = None;
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
    let outcome = run_opts(
        &store.db,
        &store.index,
        &ast,
        limit.unwrap_or(200),
        scope_id,
        Tuning::INTERACTIVE,
    )?;
    Ok(serde_json::json!({
        "items": outcome.items,
        "matched": outcome.matched,
        "matchedPending": outcome.matched_pending,
        "total": outcome.total,
        "elapsedMs": outcome.elapsed_ms,
        "ast": ast,
        "explain": ast.explain(),
    }))
}

/// 精确命中数：`cmd_search` 返回 `matchedPending` 时由前端随后调用（不限时，不取列表）。
#[tauri::command]
pub async fn cmd_search_count(
    store: tauri::State<'_, std::sync::Arc<crate::AppState>>,
    query: String,
    scope_id: Option<i64>,
) -> Result<i64, String> {
    let ast = if query.trim().is_empty() {
        crate::models::QueryAst::default()
    } else {
        crate::query::parse(&query)
    };
    Ok(run_opts(&store.db, &store.index, &ast, 0, scope_id, Tuning::EXACT)?.matched)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 空搜索只带类型筛选时是「按条件找文件」：不受默认位置根目录第一层的限制。
    #[test]
    fn filter_without_query_is_not_limited_to_root_layer() {
        crate::testutil::retry_on_index_io(|| {
            let dir = tempfile::tempdir().unwrap();
            let store = crate::settings::SettingsStore::new(crate::db::open_in_memory().unwrap());
            let index = crate::searchindex::SearchIndex::open_or_create(dir.path()).unwrap();
            store.with_conn(|c| {
                c.execute("INSERT INTO scopes(scope_id, path, added_at) VALUES (1, 'D:/a', 'now')", [])
                    .unwrap();
                for (name, kind, parent) in [("top.txt", "doc", "D:/a"), ("pic.jpg", "image", "D:/a/sub")] {
                    c.execute(
                        "INSERT INTO files(scope_id, name, path, kind, size, parent_path)
                         VALUES (1, ?1, ?2, ?3, 1, ?4)",
                        rusqlite::params![name, format!("{parent}\\{name}"), kind, parent],
                    )
                    .unwrap();
                }
            });
            // 纯浏览：只列根目录第一层
            let browse = run(&store, &index, &QueryAst::default(), 10, Some(1)).unwrap();
            assert_eq!(browse.items.len(), 1);
            // 带类型筛选：子目录里的图片也应命中
            let images = QueryAst {
                kinds: vec!["image".into()],
                ..Default::default()
            };
            let r = run(&store, &index, &images, 10, Some(1)).unwrap();
            assert_eq!(r.items.len(), 1);
            assert_eq!(r.items[0].name, "pic.jpg");
        });
    }

    /// `path:D:\Temp` 按目录边界匹配：含该目录本身与其下内容，不误中 `D:\Temp2`；大小写不敏感。
    #[test]
    fn absolute_path_filter_matches_directory_boundary() {
        crate::testutil::retry_on_index_io(|| {
            let dir = tempfile::tempdir().unwrap();
            let store = crate::settings::SettingsStore::new(crate::db::open_in_memory().unwrap());
            let index = crate::searchindex::SearchIndex::open_or_create(dir.path()).unwrap();
            store.with_conn(|c| {
                c.execute("INSERT INTO scopes(scope_id, path, added_at) VALUES (1, 'D:\\', 'now')", [])
                    .unwrap();
                for (name, parent) in [
                    ("a.txt", "D:\\Temp"),
                    ("b.txt", "D:\\Temp\\sub"),
                    ("c.txt", "D:\\Temp2"),
                    ("d.txt", "E:\\Temp"),
                ] {
                    c.execute(
                        "INSERT INTO files(scope_id, name, path, kind, size, parent_path)
                         VALUES (1, ?1, ?2, 'doc', 1, ?3)",
                        rusqlite::params![name, format!("{parent}\\{name}"), parent],
                    )
                    .unwrap();
                }
            });
            let names = |q: &str| {
                let ast = crate::query::parse(q);
                let mut v: Vec<String> = run(&store, &index, &ast, 10, None)
                    .unwrap()
                    .items
                    .into_iter()
                    .map(|i| i.name)
                    .collect();
                v.sort();
                v
            };
            assert_eq!(names(r"path:D:\Temp"), vec!["a.txt", "b.txt"]);
            assert_eq!(names(r"path:d:/temp/"), vec!["a.txt", "b.txt"]);
            assert_eq!(names(r"-path:D:\Temp"), vec!["c.txt", "d.txt"]);
            // 相对写法仍是「路径包含」：Temp2 与 E:\Temp 也会命中
            assert_eq!(names("path:Temp").len(), 4);
        });
    }

    /// 造一批带并列排序键的数据：3 个类型、大小与修改时间都有重复值，部分在子目录。
    fn seed_mixed(store: &crate::settings::SettingsStore, n: i64) {
        store.with_conn(|c| {
            c.execute("INSERT INTO scopes(scope_id, path, added_at) VALUES (1, 'D:\\', 'now')", [])
                .unwrap();
            for i in 0..n {
                let (kind, ext) = match i % 3 {
                    0 => ("image", "jpg"),
                    1 => ("doc", "docx"),
                    _ => ("pdf", "pdf"),
                };
                // 名称大小写混排，验证不分大小写的排序；size / modified 每 4 个一组并列
                let name = if i % 2 == 0 { format!("File{:03}.{ext}", n - i) } else { format!("file{:03}.{ext}", n - i) };
                let parent = if i % 5 == 0 { "D:\\data\\sub" } else { "D:\\data" };
                c.execute(
                    "INSERT INTO files(scope_id, name, path, ext, kind, size, modified_ms, parent_path)
                     VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    rusqlite::params![name, format!("{parent}\\{name}"), ext, kind, (i / 4) * 100, 1_000 + (i / 4), parent],
                )
                .unwrap();
            }
        });
    }

    /// 「边走边筛」快路径与常规计划必须返回完全相同的条目与顺序（含并列项），命中数也一致。
    #[test]
    fn interactive_scan_matches_exact_plan() {
        crate::testutil::retry_on_index_io(|| {
            let dir = tempfile::tempdir().unwrap();
            let store = crate::settings::SettingsStore::new(crate::db::open_in_memory().unwrap());
            let index = crate::searchindex::SearchIndex::open_or_create(dir.path()).unwrap();
            seed_mixed(&store, 90);
            // 预算足够大：快路径一定走完，不受机器速度影响
            let generous = Tuning { scan_budget_ms: Some(60_000), count_budget_ms: Some(60_000) };
            let ids = |o: &SearchOutcome| o.items.iter().map(|i| i.file_id).collect::<Vec<_>>();
            for query in ["type:image", "type:doc,pdf", "ext:pdf", "*.jpg", "type:image size:>500", "-type:image", "path:D:\\data\\sub", "type:audio"] {
                for sort in [SortMode::Name, SortMode::Modified, SortMode::Size, SortMode::Relevance] {
                    // limit 7：匹配多于上限（提前停）；limit 200：匹配少于上限（走完全部）
                    for limit in [7usize, 200] {
                        let mut ast = crate::query::parse(query);
                        ast.sort = sort;
                        let exact = run(&store, &index, &ast, limit, None).unwrap();
                        let fast = run_opts(&store, &index, &ast, limit, None, generous).unwrap();
                        let ctx = format!("query={query} sort={sort:?} limit={limit}");
                        assert_eq!(ids(&fast), ids(&exact), "条目与顺序应一致：{ctx}");
                        assert_eq!(fast.matched, exact.matched, "命中数应一致：{ctx}");
                        assert!(!fast.matched_pending, "预算充足不应待定：{ctx}");
                    }
                }
            }
        });
    }

    /// 时间预算耗尽：列表退回常规计划（结果仍然正确），命中数标记为待定并以已列出条数为下限；
    /// 随后的精确统计给出真实命中数。
    #[test]
    fn exhausted_budget_falls_back_and_marks_count_pending() {
        crate::testutil::retry_on_index_io(|| {
            let dir = tempfile::tempdir().unwrap();
            let store = crate::settings::SettingsStore::new(crate::db::open_in_memory().unwrap());
            let index = crate::searchindex::SearchIndex::open_or_create(dir.path()).unwrap();
            seed_mixed(&store, 90);
            let mut ast = crate::query::parse("type:image");
            ast.sort = SortMode::Modified;
            let exact = run(&store, &index, &ast, 7, None).unwrap();
            assert_eq!(exact.matched, 30);
            // 预算 0：进度回调在第一条指令后即中断，相当于「立刻超时」
            let starved = Tuning { scan_budget_ms: Some(0), count_budget_ms: Some(0) };
            let out = run_opts(&store, &index, &ast, 7, None, starved).unwrap();
            let ids = |o: &SearchOutcome| o.items.iter().map(|i| i.file_id).collect::<Vec<_>>();
            assert_eq!(ids(&out), ids(&exact), "超时后退回常规计划，结果不变");
            assert!(out.matched_pending, "计数超时应标记待定");
            assert_eq!(out.matched, 7, "待定时以已列出条数为下限");
            // 后台精确统计（limit 0，不取列表）
            let counted = run_opts(&store, &index, &ast, 0, None, Tuning::EXACT).unwrap();
            assert!(counted.items.is_empty());
            assert_eq!(counted.matched, 30);
            assert!(!counted.matched_pending);
        });
    }

    /// 空搜索浏览根目录第一层走专用复合索引，不应尝试边走边筛（否则稀疏条件下每次白付预算）。
    #[test]
    fn first_layer_browse_is_unaffected_by_interactive_tuning() {
        crate::testutil::retry_on_index_io(|| {
            let dir = tempfile::tempdir().unwrap();
            let store = crate::settings::SettingsStore::new(crate::db::open_in_memory().unwrap());
            let index = crate::searchindex::SearchIndex::open_or_create(dir.path()).unwrap();
            store.with_conn(|c| {
                c.execute("INSERT INTO scopes(scope_id, path, added_at) VALUES (1, 'D:/a', 'now')", [])
                    .unwrap();
                for (name, parent) in [("b.txt", "D:/a"), ("A.txt", "D:/a"), ("deep.txt", "D:/a/sub")] {
                    c.execute(
                        "INSERT INTO files(scope_id, name, path, kind, size, parent_path) VALUES (1, ?1, ?2, 'doc', 1, ?3)",
                        rusqlite::params![name, format!("{parent}/{name}"), parent],
                    )
                    .unwrap();
                }
            });
            let ast = QueryAst { sort: SortMode::Name, ..Default::default() };
            let starved = Tuning { scan_budget_ms: Some(0), count_budget_ms: Some(0) };
            let out = run_opts(&store, &index, &ast, 10, Some(1), starved).unwrap();
            let names: Vec<&str> = out.items.iter().map(|i| i.name.as_str()).collect();
            assert_eq!(names, vec!["A.txt", "b.txt"], "不分大小写排序，且只列第一层");
            assert!(!out.matched_pending, "纯浏览不统计命中数，不会待定");
        });
    }

    #[test]
    fn deindex_keeps_sort_column_indexable() {
        assert_eq!(deindex("f.kind IN (?,?)", "f.modified_ms"), "+f.kind IN (?,?)");
        assert_eq!(deindex("f.modified_ms >= ?", "f.modified_ms"), "f.modified_ms >= ?");
        assert_eq!(deindex("f.modified_ms >= ?", "f.size"), "+f.modified_ms >= ?");
        assert_eq!(
            deindex("f.name NOT LIKE ? ESCAPE '\\' AND f.ext <> ?", "f.size"),
            "+f.name NOT LIKE ? ESCAPE '\\' AND +f.ext <> ?"
        );
        // parent_path 与 path 是两个不同的列，互不误伤
        assert_eq!(deindex("f.parent_path = ? AND f.path LIKE ?", "f.name"), "+f.parent_path = ? AND +f.path LIKE ?");
        assert_eq!(deindex("f.scope_id IN (SELECT scope_id FROM scopes WHERE enabled=1)", "f.name"), "+f.scope_id IN (SELECT scope_id FROM scopes WHERE enabled=1)");
    }

    /// 已关闭的索引范围不参与搜索与计数；重新启用后恢复。
    #[test]
    fn disabled_scope_is_excluded_from_results_and_total() {
        crate::testutil::retry_on_index_io(|| {
            let dir = tempfile::tempdir().unwrap();
            let store = crate::settings::SettingsStore::new(crate::db::open_in_memory().unwrap());
            let index = crate::searchindex::SearchIndex::open_or_create(dir.path()).unwrap();
            store.with_conn(|c| {
                c.execute(
                    "INSERT INTO scopes(scope_id, path, added_at) VALUES (1, 'D:/a', 'now'), (2, 'D:/b', 'now')",
                    [],
                )
                .unwrap();
                for (sid, name, parent) in [(1, "alpha.txt", "D:/a"), (2, "beta.txt", "D:/b")] {
                    c.execute(
                        "INSERT INTO files(scope_id, name, path, kind, size, parent_path)
                         VALUES (?1, ?2, ?3, 'doc', 1, ?4)",
                        rusqlite::params![sid, name, format!("{parent}\\{name}"), parent],
                    )
                    .unwrap();
                }
            });
            let ast = QueryAst::default();
            let names = |o: &SearchOutcome| {
                let mut v: Vec<String> = o.items.iter().map(|i| i.name.clone()).collect();
                v.sort();
                v
            };
            invalidate_total_cache();
            let all = run(&store, &index, &ast, 10, None).unwrap();
            assert_eq!(names(&all), vec!["alpha.txt", "beta.txt"]);
            assert_eq!(all.total, 2);

            store.with_conn(|c| c.execute("UPDATE scopes SET enabled=0 WHERE scope_id=2", []).unwrap());
            invalidate_total_cache();
            let off = run(&store, &index, &ast, 10, None).unwrap();
            assert_eq!(names(&off), vec!["alpha.txt"]);
            assert_eq!(off.total, 1);

            store.with_conn(|c| c.execute("UPDATE scopes SET enabled=1 WHERE scope_id=2", []).unwrap());
            invalidate_total_cache();
            let on = run(&store, &index, &ast, 10, None).unwrap();
            assert_eq!(names(&on), vec!["alpha.txt", "beta.txt"]);
        });
    }

    /// 空搜索浏览：scope_id 限定范围且仅列根目录第一层，名称与大小排序保持稳定。
    #[test]
    fn scope_filter_and_browse_sorts() {
        crate::testutil::retry_on_index_io(|| {
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
        });
    }

    /// 「共 N 项」必须是真实命中数：结果被 200 条上限截断时，
    /// 界面要能告诉用户实际命中了多少条，而不是只报列出的条数。
    #[test]
    fn matched_counts_all_hits_not_just_returned_page() {
        crate::testutil::retry_on_index_io(|| {
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
        });
    }

    /// 多个关键词是「且」：只含其中一个词的文件不得出现在结果里。
    #[test]
    fn multiple_keywords_must_all_match() {
        crate::testutil::retry_on_index_io(|| {
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
            for name in ["年度报告2026.pdf", "年度报告2025.pdf", "旅行计划2026.docx"] {
                let fid = store.with_conn(|c| {
                    c.execute(
                        "INSERT INTO files(scope_id, name, path, kind, size, parent_path)
                         VALUES (1, ?1, ?2, 'doc', 1, 'D:/a')",
                        rusqlite::params![name, format!("D:/a\\{name}")],
                    )
                    .unwrap();
                    c.last_insert_rowid() as u64
                });
                index.add(index.build_doc(fid, name, None, None)).unwrap();
            }
            index.commit().unwrap();
            index.reload();

            let ast = QueryAst {
                text: Some("报告 2026".into()),
                ..Default::default()
            };
            let out = run(&store, &index, &ast, 20, None).unwrap();
            let names: Vec<&str> = out.items.iter().map(|i| i.name.as_str()).collect();
            assert_eq!(names, vec!["年度报告2026.pdf"], "两个词都要命中");
        });
    }

    /// 正文检索：查询词要按 jieba 切成词项再匹配，整串当作单个词项会让长词永远命不中。
    #[test]
    fn content_query_is_segmented_like_indexed_text() {
        crate::testutil::retry_on_index_io(|| {
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
            let fid = store.with_conn(|c| {
                c.execute(
                    "INSERT INTO files(scope_id, name, path, kind, size, parent_path)
                     VALUES (1, 'notes.txt', 'D:/a\\notes.txt', 'doc', 1, 'D:/a')",
                    [],
                )
                .unwrap();
                c.last_insert_rowid() as u64
            });
            index
                .add(index.build_doc(fid, "notes.txt", Some("本季度的项目管理报告详见附录"), None))
                .unwrap();
            index.commit().unwrap();
            index.reload();

            let ast = QueryAst {
                text: Some("项目管理报告".into()),
                ..Default::default()
            };
            let out = run(&store, &index, &ast, 20, None).unwrap();
            assert_eq!(out.items.len(), 1, "文件名不含该词，应由正文命中");
        });
    }

    /// 排除词里的 `_` `%` 是字面量，不是 LIKE 通配。
    #[test]
    fn exclude_text_treats_like_metachars_literally() {
        crate::testutil::retry_on_index_io(|| {
            let dir = tempfile::tempdir().unwrap();
            let store = crate::settings::SettingsStore::new(crate::db::open_in_memory().unwrap());
            let index = crate::searchindex::SearchIndex::open_or_create(dir.path()).unwrap();
            store.with_conn(|c| {
                c.execute(
                    "INSERT INTO scopes(scope_id, path, added_at) VALUES (1, 'D:/a', 'now')",
                    [],
                )
                .unwrap();
                for name in ["a_draft.txt", "abdraft.txt"] {
                    c.execute(
                        "INSERT INTO files(scope_id, name, path, kind, size, parent_path)
                         VALUES (1, ?1, ?2, 'doc', 1, 'D:/a')",
                        rusqlite::params![name, format!("D:/a\\{name}")],
                    )
                    .unwrap();
                }
            });
            let ast = QueryAst {
                text_exclude: vec!["_draft".into()],
                ..Default::default()
            };
            let out = run(&store, &index, &ast, 20, None).unwrap();
            let names: Vec<&str> = out.items.iter().map(|i| i.name.as_str()).collect();
            assert_eq!(names, vec!["abdraft.txt"], "只排除字面含 _draft 的文件");
        });
    }

    /// 通配模式按整名匹配：`*` 任意多字符、`?` 单字符，且元字符被转义。
    #[test]
    fn wildcard_filters_by_full_name() {
        crate::testutil::retry_on_index_io(|| {
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
                        "INSERT INTO files(scope_id, name, path, ext, kind, size, parent_path)
                         VALUES (1, ?1, ?2, ?3, 'doc', 1, 'D:/a')",
                        // 扫描器写入的 ext 是小写、无点的扩展名；`*.pdf` 走 ext 索引依赖它
                        rusqlite::params![
                            name,
                            format!("D:/a\\{name}"),
                            name.rsplit('.').next().unwrap_or("")
                        ],
                    )
                    .unwrap();
                }
            });

            let names =|ast: &QueryAst| -> Vec<String> {
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
        });
    }

    /// 通配查询不应被当作空搜索浏览（不受 scope_id 的第一层限制）。
    #[test]
    fn wildcard_is_not_treated_as_empty_browse() {
        crate::testutil::retry_on_index_io(|| {
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
                    "INSERT INTO files(scope_id, name, path, ext, kind, size, parent_path)
                     VALUES (1, 'deep.pdf', 'D:/a/sub\\deep.pdf', 'pdf', 'doc', 1, 'D:/a/sub')",
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
        });
    }
}
