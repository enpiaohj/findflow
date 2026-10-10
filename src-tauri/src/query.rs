use crate::models::QueryAst;

/// 解析筛选语法（设计文档附录 A）：
/// `ext:docx` `type:pdf` `path:x` `size:>1GB` `modified:7d` `-排除` `"短语"`
pub fn parse(input: &str) -> QueryAst {
    let mut ast = QueryAst::default();
    let mut free: Vec<String> = Vec::new();
    let mut phrase_parts: Vec<String> = Vec::new();
    let mut chars = input.chars().peekable();
    let mut buf = String::new();

    while let Some(c) = chars.next() {
        match c {
            '"' => {
                // 引号短语；短语内若含通配符则按文件名整名匹配
                let mut p = String::new();
                for c2 in chars.by_ref() {
                    if c2 == '"' {
                        break;
                    }
                    p.push(c2);
                }
                if !p.is_empty() {
                    if is_wildcard(&p) {
                        ast.name_include.push(p);
                    } else {
                        phrase_parts.push(p);
                    }
                }
            }
            ' ' => {
                flush_token(&buf, &mut free, &mut ast);
                buf.clear();
            }
            _ => buf.push(c),
        }
    }
    flush_token(&buf, &mut free, &mut ast);

    if !phrase_parts.is_empty() {
        ast.phrase = Some(phrase_parts.join(" "));
    }
    let text = free.join(" ");
    if !text.is_empty() {
        ast.text = Some(text);
    }
    ast
}

fn flush_token(tok: &str, free: &mut Vec<String>, ast: &mut QueryAst) {
    let t = tok.trim();
    if t.is_empty() {
        return;
    }
    let (neg, t) = match t.strip_prefix('-') {
        Some(rest) if rest.len() > 1 => (true, rest),
        _ => (false, t),
    };
    if let Some((key, value)) = t.split_once(':') {
        let v = value.trim();
        if v.is_empty() {
            return;
        }
        match key.to_ascii_lowercase().as_str() {
            "ext" | "扩展名" => {
                for e in v.split([',', '，']) {
                    let e = e.trim().trim_start_matches('.').to_lowercase();
                    if !e.is_empty() {
                        if neg {
                            ast.text_exclude.push(format!(".{e}"));
                        } else {
                            ast.exts.push(e);
                        }
                    }
                }
                return;
            }
            "type" | "kind" | "类型" => {
                // 「文档」是一组类型而非单一类型：Word、Excel、PDF、纯文本都算文档。
                // 只映射到 doc 会让「文档」筛选漏掉 Excel 与 PDF——用户反馈搜索到的
                // 表格文件点「文档」后整个消失。需要单一类型时用 type:doc / type:sheet。
                if matches!(v.to_lowercase().as_str(), "document" | "文档") {
                    let group = ["doc", "sheet", "pdf"].map(String::from);
                    if neg {
                        ast.kinds_exclude.extend(group);
                    } else {
                        ast.kinds.extend(group);
                    }
                    return;
                }
                let mapped = map_kind(v);
                if neg {
                    ast.kinds_exclude.push(mapped);
                } else {
                    ast.kinds.push(mapped);
                }
                return;
            }
            "path" | "路径" => {
                if neg {
                    ast.path_exclude.push(v.to_string());
                } else {
                    ast.path_include.push(v.to_string());
                }
                return;
            }
            "size" | "大小" => {
                let (min, max) = parse_size(v);
                ast.size_min = min;
                ast.size_max = max;
                return;
            }
            "modified" | "修改时间" | "date" => {
                let (after, before) = parse_time(v);
                ast.modified_after_ms = after;
                ast.modified_before_ms = before;
                return;
            }
            _ => {}
        }
    }
    // 含通配符的自由词按整名匹配（`*.pdf`、`报告*.docx`），
    // 不走全文检索——`*` 在分词与倒排里无法表达「任意字符」。
    if neg {
        if is_wildcard(t) {
            ast.name_exclude.push(t.to_string());
        } else {
            ast.text_exclude.push(t.to_string());
        }
    } else if is_wildcard(t) {
        ast.name_include.push(t.to_string());
    } else {
        free.push(t.to_string());
    }
}

/// 自由词是否含通配符（`*` 任意多字符、`?` 单字符）。
pub fn is_wildcard(token: &str) -> bool {
    token.contains('*') || token.contains('?')
}

/// 通配模式 → SQL LIKE 模式：`*` → `%`、`?` → `_`，
/// 并把模式中原本的 LIKE 元字符（`%` `_` 与转义符）转义，避免用户输入被当作通配。
/// 返回的模式需配合 `LIKE ? ESCAPE '\'` 使用。
pub fn wildcard_to_like(pattern: &str) -> String {
    let mut out = String::with_capacity(pattern.len() + 4);
    for c in pattern.chars() {
        match c {
            '*' => out.push('%'),
            '?' => out.push('_'),
            // 反斜杠是 ESCAPE 字符，必须先转义自身
            '\\' => out.push_str("\\\\"),
            '%' => out.push_str("\\%"),
            '_' => out.push_str("\\_"),
            _ => out.push(c),
        }
    }
    out
}

fn map_kind(v: &str) -> String {
    match v.to_lowercase().as_str() {
        "document" | "doc" | "文档" => "doc".into(),
        "image" | "图片" | "照片" => "image".into(),
        "video" | "视频" => "video".into(),
        "audio" | "音频" => "audio".into(),
        "archive" | "压缩包" => "archive".into(),
        "folder" | "文件夹" | "目录" => "folder".into(),
        "sheet" | "表格" => "sheet".into(),
        "pdf" => "pdf".into(),
        other => other.to_string(),
    }
}

/// `>1GB` `<=10MB` `2GB..4GB` `500KB`
fn parse_size(v: &str) -> (Option<i64>, Option<i64>) {
    const KB: i64 = 1024;
    const MB: i64 = KB * 1024;
    const GB: i64 = MB * 1024;
    const TB: i64 = GB * 1024;
    let unit_size = |s: &str| -> Option<i64> {
        let s = s.trim();
        let (num, unit) = s.split_at(s.find(|c: char| c.is_ascii_alphabetic()).unwrap_or(s.len()));
        let n: f64 = num.trim().parse().ok()?;
        let m = match unit.to_ascii_uppercase().as_str() {
            "KB" => KB,
            "MB" => MB,
            "GB" => GB,
            "TB" => TB,
            "B" | "" => 1,
            _ => 1,
        };
        Some((n * m as f64) as i64)
    };
    if let Some((a, b)) = v.split_once("..") {
        return (unit_size(a), unit_size(b));
    }
    if let Some(rest) = v.strip_prefix('>') {
        return (unit_size(rest), None);
    }
    if let Some(rest) = v.strip_prefix(">=") {
        return (unit_size(rest), None);
    }
    if let Some(rest) = v.strip_prefix('<') {
        return (None, unit_size(rest));
    }
    if let Some(rest) = v.strip_prefix("<=") {
        return (None, unit_size(rest));
    }
    let n = unit_size(v);
    (n, n)
}

/// `7d` `3h` `2026-09-01..2026-09-21` `>2026-09-01`
fn parse_time(v: &str) -> (Option<i64>, Option<i64>) {
    let now = chrono::Utc::now();
    if let Some(days) = v.strip_suffix('d').and_then(|n| n.parse::<i64>().ok()) {
        let after = now - chrono::Duration::days(days);
        return (Some(after.timestamp_millis()), None);
    }
    if let Some(hours) = v.strip_suffix('h').and_then(|n| n.parse::<i64>().ok()) {
        let after = now - chrono::Duration::hours(hours);
        return (Some(after.timestamp_millis()), None);
    }
    if let Some(months) = v.strip_suffix("mo").and_then(|n| n.parse::<i64>().ok()) {
        let after = now - chrono::Duration::days(30 * months);
        return (Some(after.timestamp_millis()), None);
    }
    let parse_day = |s: &str| -> Option<i64> {
        chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .ok()?
            .and_hms_opt(0, 0, 0)?
            .and_utc()
            .timestamp_millis()
            .into()
    };
    if let Some((a, b)) = v.split_once("..") {
        return (parse_day(a), parse_day(b).map(|t| t + 86_399_000));
    }
    if let Some(rest) = v.strip_prefix('>') {
        return (parse_day(rest), None);
    }
    if let Some(rest) = v.strip_prefix('<') {
        return (None, parse_day(rest));
    }
    (parse_day(v), parse_day(v).map(|t| t + 86_399_000))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_text() {
        let ast = parse("AD升级 方案");
        assert_eq!(ast.text.as_deref(), Some("AD升级 方案"));
        assert!(!ast.has_filters());
    }

    #[test]
    fn parses_ext_and_path() {
        let ast = parse("ext:docx,pdf AD升级 path:项目");
        assert_eq!(ast.exts, vec!["docx", "pdf"]);
        assert_eq!(ast.path_include, vec!["项目"]);
        assert_eq!(ast.text.as_deref(), Some("AD升级"));
    }

    #[test]
    fn parses_size_range_and_time() {
        let ast = parse("size:>1GB modified:7d");
        assert!(ast.size_min.unwrap() > 1024 * 1024 * 1024 - 10);
        assert!(ast.modified_after_ms.is_some());
        let ast2 = parse("modified:2026-09-01..2026-09-21");
        assert!(ast2.modified_before_ms.unwrap() > ast2.modified_after_ms.unwrap());
    }

    #[test]
    fn parses_exclusions_and_phrase() {
        let ast = parse("方案 -path:备份 \"实施方案\" -测试");
        assert_eq!(ast.path_exclude, vec!["备份"]);
        assert_eq!(ast.phrase.as_deref(), Some("实施方案"));
        assert_eq!(ast.text_exclude, vec!["测试"]);
    }

    #[test]
    fn document_type_filter_covers_office_group() {
        // 「文档」按钮要能筛出 Excel 与 PDF，只映射 doc 会让表格文件整个消失
        let ast = parse("type:document");
        assert_eq!(ast.kinds, vec!["doc", "sheet", "pdf"]);
        assert_eq!(parse("type:文档").kinds, vec!["doc", "sheet", "pdf"]);
        // 单一类型仍可用，且与组别名区分
        assert_eq!(parse("type:doc").kinds, vec!["doc"]);
        assert_eq!(parse("type:sheet").kinds, vec!["sheet"]);
        // 取反走类型排除，不落到按文件名排除
        let neg = parse("-type:文档");
        assert_eq!(neg.kinds_exclude, vec!["doc", "sheet", "pdf"]);
        assert!(neg.kinds.is_empty());
        assert!(
            neg.text_exclude.is_empty(),
            "类型取反不能落到 text_exclude（那里按文件名子串排除）"
        );
        let neg2 = parse("-type:图片");
        assert_eq!(neg2.kinds_exclude, vec!["image"]);
        assert!(neg2.text_exclude.is_empty());
    }

    #[test]
    fn parses_type_and_size_range() {
        let ast = parse("type:图片 size:1GB..2GB");
        assert_eq!(ast.kinds, vec!["image"]);
        assert!(ast.size_min.unwrap() > 0 && ast.size_max.unwrap() >= ast.size_min.unwrap());
    }

    #[test]
    fn wildcard_tokens_go_to_name_match_not_fulltext() {
        let ast = parse("*.pdf");
        assert!(ast.text.is_none(), "通配词不应进入全文检索");
        assert_eq!(ast.name_include, vec!["*.pdf"]);
        assert!(ast.has_filters());

        let ast2 = parse("报告*.docx 方案");
        assert_eq!(ast2.name_include, vec!["报告*.docx"]);
        assert_eq!(ast2.text.as_deref(), Some("方案"), "普通词仍走全文检索");

        let ast3 = parse("2026-??-??.txt");
        assert_eq!(ast3.name_include, vec!["2026-??-??.txt"]);
    }

    #[test]
    fn wildcard_with_negation_excludes_by_name() {
        let ast = parse("*.log -*.tmp");
        assert_eq!(ast.name_include, vec!["*.log"]);
        assert_eq!(ast.name_exclude, vec!["*.tmp"]);
        assert!(ast.text_exclude.is_empty());
    }

    #[test]
    fn wildcard_inside_quotes_matches_name() {
        let ast = parse("\"报告*.docx\"");
        assert_eq!(ast.name_include, vec!["报告*.docx"]);
        assert!(ast.phrase.is_none(), "含通配符的引号短语不走短语检索");
        // 不含通配符的引号短语语义不变
        let ast2 = parse("\"实施方案\"");
        assert_eq!(ast2.phrase.as_deref(), Some("实施方案"));
        assert!(ast2.name_include.is_empty());
    }

    #[test]
    fn wildcard_to_like_translates_and_escapes() {
        assert_eq!(wildcard_to_like("*.pdf"), "%.pdf");
        assert_eq!(wildcard_to_like("报告*.docx"), "报告%.docx");
        assert_eq!(wildcard_to_like("2026-??-??.txt"), "2026-__-__.txt");
        // LIKE 元字符必须转义，避免用户输入的 % _ 变成通配
        assert_eq!(wildcard_to_like("100%_done*"), "100\\%\\_done%");
        // 反斜杠自身先转义（ESCAPE '\'）
        assert_eq!(wildcard_to_like("a\\b*"), "a\\\\b%");
    }

    #[test]
    fn plain_tokens_unaffected_by_wildcard_support() {
        let ast = parse("AD升级 ext:docx");
        assert_eq!(ast.text.as_deref(), Some("AD升级"));
        assert!(ast.name_include.is_empty());
        assert_eq!(ast.exts, vec!["docx"]);
    }
}
