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
                // 引号短语
                let mut p = String::new();
                for c2 in chars.by_ref() {
                    if c2 == '"' {
                        break;
                    }
                    p.push(c2);
                }
                if !p.is_empty() {
                    phrase_parts.push(p);
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
                let mapped = map_kind(v);
                if neg {
                    ast.text_exclude.push(mapped);
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
    if neg {
        ast.text_exclude.push(t.to_string());
    } else {
        free.push(t.to_string());
    }
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
    fn parses_type_and_size_range() {
        let ast = parse("type:图片 size:1GB..2GB");
        assert_eq!(ast.kinds, vec!["image"]);
        assert!(ast.size_min.unwrap() > 0 && ast.size_max.unwrap() >= ast.size_min.unwrap());
    }
}
