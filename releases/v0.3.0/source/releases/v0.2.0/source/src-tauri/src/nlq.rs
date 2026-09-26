use crate::models::QueryAst;

/// 离线自然语言解析（FR16）：确定性表达 → 可见结构化条件，不执行任何文件操作。
pub fn parse_nl(input: &str) -> (QueryAst, f64) {
    let mut ast = QueryAst::default();
    let lower = input.to_lowercase();
    let mut confidence = 0.0f64;

    // 时间词
    let now = chrono::Utc::now();
    let day = |n: i64| (now - chrono::Duration::days(n)).timestamp_millis();
    if lower.contains("今天") {
        ast.modified_after_ms = Some(day(1));
        confidence += 0.3;
    } else if lower.contains("昨天") {
        ast.modified_after_ms = Some(day(2));
        ast.modified_before_ms = Some(day(1));
        confidence += 0.3;
    } else if lower.contains("本周") || lower.contains("这周") {
        ast.modified_after_ms = Some(day(7));
        confidence += 0.3;
    } else if lower.contains("上个月") {
        ast.modified_after_ms = Some(day(31));
        confidence += 0.25;
    } else if let Some(n) = extract_recent_days(&lower) {
        ast.modified_after_ms = Some(day(n));
        confidence += 0.3;
    }

    // 大小词
    let (size_min, size_max) = extract_size(&lower);
    ast.size_min = size_min;
    ast.size_max = size_max;
    if size_min.is_some() || size_max.is_some() {
        confidence += 0.3;
    }

    // 类型词
    let kind_map: &[(&str, &str)] = &[
        ("word", "doc"), ("文档", "doc"), ("excel", "sheet"), ("表格", "sheet"),
        ("ppt", "doc"), ("pdf", "pdf"), ("图片", "image"), ("照片", "image"),
        ("视频", "video"), ("音频", "audio"), ("压缩包", "archive"),
        ("文件夹", "folder"), ("安装包", "other"),
    ];
    for (kw, kind) in kind_map {
        if lower.contains(kw) {
            if *kw == "安装包" {
                ast.exts.push("msi".into());
                ast.exts.push("exe".into());
            } else {
                ast.kinds.push(kind.to_string());
            }
            confidence += 0.3;
            break;
        }
    }
    // 扩展名词
    for token in lower.split_whitespace() {
        let t = token.trim_start_matches('.');
        if t.len() >= 2 && t.chars().all(|c| c.is_ascii_alphanumeric()) {
            ast.exts.push(t.to_string());
            confidence += 0.2;
            break;
        }
    }

    // 位置词
    const PLACES: &[(&str, &str)] = &[
        ("d盘", "D:\\"), ("c盘", "C:\\"), ("e盘", "E:\\"),
        ("下载", "下载"), ("桌面", "桌面"), ("文档", "文档"), ("图片", "图片"),
    ];
    for (kw, prefix) in PLACES {
        if lower.contains(kw) {
            ast.path_include.push(prefix.to_string());
            confidence += 0.2;
            break;
        }
    }

    // 剩余文本作为关键词（去掉已识别的表达）
    let mut residual = input.to_string();
    for kw in ["今天", "昨天", "本周", "这周", "上个月", "修改的", "大于", "小于", "超过", "最近的", "最近"] {
        residual = residual.replace(kw, "");
    }
    let residual = residual.trim().to_string();
    if !residual.is_empty() && confidence > 0.0 {
        ast.text = Some(residual);
    } else if confidence == 0.0 {
        // 完全没有确定性表达 → 回退普通关键词（FR16 闭环）
        ast.text = Some(input.trim().to_string());
        return (ast, 0.0);
    }
    (ast, confidence.min(1.0))
}

fn extract_recent_days(s: &str) -> Option<i64> {
    if let Some(pos) = s.find("最近") {
        let rest = &s[pos + "最近".len()..];
        let num: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Ok(n) = num.parse::<i64>() {
            let rest2 = &rest[num.len()..];
            if rest2.starts_with("天") {
                return Some(n);
            }
            if rest2.starts_with("周") {
                return Some(n * 7);
            }
        }
        if rest.starts_with("一周") {
            return Some(7);
        }
        return Some(7);
    }
    None
}

fn extract_size(s: &str) -> (Option<i64>, Option<i64>) {
    let find = |kw: &str| -> Option<(bool, i64)> {
        let pos = s.find(kw)?;
        let rest = &s[pos + kw.len()..];
        let num: String = rest
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect();
        let n: f64 = num.parse().ok()?;
        let unit_bytes = if rest.contains("gb") || rest.contains("g）") {
            1024 * 1024 * 1024
        } else if rest.contains("mb") {
            1024 * 1024
        } else if rest.contains("kb") {
            1024
        } else {
            1024 * 1024 * 1024
        };
        Some((kw.contains("小"), (n * unit_bytes as f64) as i64))
    };
    if let Some((_is_less, bytes)) = find("小于") {
        return (None, Some(bytes));
    }
    if let Some((_, bytes)) = find("大于").or_else(|| find("超过")) {
        return (Some(bytes), None);
    }
    let _ = find("小于");
    (None, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_today_modified_word() {
        let (ast, conf) = parse_nl("今天修改的 Word");
        assert!(ast.modified_after_ms.is_some());
        assert_eq!(ast.kinds, vec!["doc"]);
        assert!(conf > 0.3);
    }

    #[test]
    fn parses_d_disk_size() {
        let (ast, _) = parse_nl("D盘大于2GB的视频");
        assert_eq!(ast.path_include, vec!["D:\\"]);
        assert!(ast.size_min.unwrap() > 1024 * 1024 * 1024);
        assert_eq!(ast.kinds, vec!["video"]);
    }

    #[test]
    fn falls_back_to_keywords() {
        let (ast, conf) = parse_nl("随便找个东西");
        assert_eq!(ast.text.as_deref(), Some("随便找个东西"));
        assert!(conf < 0.01);
    }

    #[test]
    fn parses_installer() {
        let (ast, _) = parse_nl("最近下载的安装包");
        assert!(ast.exts.contains(&"msi".to_string()));
        assert!(ast.modified_after_ms.is_some());
    }
}

/// 离线自然语言解析命令（confidence 低于阈值时前端提示回退）。
#[tauri::command]
pub fn cmd_nl_parse(text: String) -> Result<serde_json::Value, String> {
    let (ast, confidence) = parse_nl(&text);
    Ok(serde_json::json!({
        "ast": ast,
        "confidence": confidence,
        "explain": ast.explain(),
    }))
}
