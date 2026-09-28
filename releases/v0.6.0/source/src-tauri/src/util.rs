use std::path::Path;
use std::sync::OnceLock;

/// 应用自身数据目录（数据库 / 索引 / 日志，小写归一化）。扫描与监听必须跳过，
/// 否则范围包含数据目录时，自身写入会被监听回灌入库，形成无限循环。
static APP_DATA_DIR: OnceLock<String> = OnceLock::new();

pub fn set_app_data_dir(p: &Path) {
    let _ = APP_DATA_DIR.set(normalize_path(p).trim_end_matches('\\').to_lowercase());
}

/// 路径是否位于应用数据目录内（含目录本身）。
pub fn is_app_data_path(path: &str) -> bool {
    let Some(dir) = APP_DATA_DIR.get() else {
        return false;
    };
    is_under(&path.to_lowercase(), dir)
}

/// `path` 是否等于 `dir` 或位于其下（两者均为小写、无尾部分隔符）。
fn is_under(path: &str, dir: &str) -> bool {
    path == dir
        || (path.len() > dir.len()
            && path.starts_with(dir)
            && path.as_bytes()[dir.len()] == b'\\')
}

/// 目录子树的路径区间：`path >= 'dir\' AND path < 'dir]'` 可走 path 唯一索引，
/// 替代 `LIKE 'dir\%'` 的全表扫描（`]` 是 `\` 的下一个字符），且不受 `_`/`%` 通配干扰。
pub fn subtree_bounds(dir: &str) -> (String, String) {
    let base = dir.trim_end_matches('\\');
    (format!("{base}\\"), format!("{base}]"))
}

/// 从扩展名推断文件大类，用于筛选与图标。
pub fn kind_from(ext: &str, is_dir: bool) -> &'static str {
    if is_dir {
        return "folder";
    }
    match ext.to_ascii_lowercase().as_str() {
        "doc" | "docx" | "txt" | "md" | "rtf" | "vsd" | "vsdx" | "html" | "htm" | "log" => "doc",
        "xls" | "xlsx" | "csv" | "ods" => "sheet",
        "pdf" => "pdf",
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "svg" | "ico" | "tif" | "tiff" => "image",
        "mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "webm" | "m4v" => "video",
        "mp3" | "wav" | "flac" | "aac" | "ogg" | "m4a" | "wma" => "audio",
        "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" => "archive",
        _ => "other",
    }
}

/// 归一化为不带 `\\?\` 前缀的常规路径，供显示与数据库存储。
pub fn normalize_path(p: &Path) -> String {
    dunce::simplified(p).to_string_lossy().replace('/', "\\")
}

/// 组合 `\\?\` 前缀的长路径，绕过 MAX_PATH 限制。
pub fn verbatim(p: &Path) -> std::path::PathBuf {
    let s = p.to_string_lossy();
    if s.starts_with("\\\\?\\") || s.starts_with("\\\\") {
        p.to_path_buf()
    } else {
        std::path::PathBuf::from(format!("\\\\?\\{}", s))
    }
}

pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

pub fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

// 文件属性位（std::os::windows::fs::MetadataExt::file_attributes）。
const ATTR_DIRECTORY: u32 = 0x10;
const ATTR_HIDDEN: u32 = 0x2;
const ATTR_SYSTEM: u32 = 0x4;
// OneDrive 占位文件：内容在云端，读取会触发下载。
const ATTR_RECALL_ON_DATA_ACCESS: u32 = 0x400000;

pub fn is_dir(attrs: u32) -> bool {
    attrs & ATTR_DIRECTORY != 0
}

pub fn is_hidden(attrs: u32) -> bool {
    attrs & ATTR_HIDDEN != 0 || attrs & ATTR_SYSTEM != 0
}

pub fn is_cloud_placeholder(attrs: u32) -> bool {
    attrs & ATTR_RECALL_ON_DATA_ACCESS != 0
}

/// 拼音首字母串（非 ASCII 字符转换，其余原样小写）。
pub fn pinyin_abbr(name: &str) -> String {
    use pinyin::ToPinyin;
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if let Some(py) = c.to_pinyin() {
            if let Some(first) = py.plain().chars().next() {
                out.push(first);
            }
        }
    }
    out
}

/// 全拼串（连写），用于中文全拼匹配。
pub fn pinyin_full(name: &str) -> String {
    use pinyin::ToPinyin;
    let mut out = String::with_capacity(name.len() * 2);
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if let Some(py) = c.to_pinyin() {
            out.push_str(py.plain());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_mapping() {
        assert_eq!(kind_from("docx", false), "doc");
        assert_eq!(kind_from("PDF", false), "pdf");
        assert_eq!(kind_from("", true), "folder");
        assert_eq!(kind_from("xyz", false), "other");
    }

    #[test]
    fn pinyin_helpers() {
        assert_eq!(pinyin_abbr("项目资料"), "xmzl");
        assert_eq!(pinyin_full("AD升级"), "adshengji");
    }

    #[test]
    fn subtree_bounds_cover_children_only() {
        let (lo, hi) = subtree_bounds(r"D:\a");
        let inside = |p: &str| p >= lo.as_str() && p < hi.as_str();
        assert!(inside(r"D:\a\b.txt"));
        assert!(inside(r"D:\a\x\y"));
        assert!(!inside(r"D:\a"));
        assert!(!inside(r"D:\ab\c"));
        assert!(!inside(r"D:\a_b"));
        assert_eq!(subtree_bounds(r"D:\").0, r"D:\");
    }

    #[test]
    fn is_under_respects_separator_boundary() {
        assert!(is_under(r"d:\findflowdata", r"d:\findflowdata"));
        assert!(is_under(r"d:\findflowdata\index\x", r"d:\findflowdata"));
        assert!(!is_under(r"d:\findflowdata2\x", r"d:\findflowdata"));
        assert!(!is_under(r"d:\find", r"d:\findflowdata"));
    }

    #[test]
    fn normalize_strips_verbatim() {
        assert_eq!(normalize_path(Path::new("D:\\\\a\\\\b.txt")), "D:\\\\a\\\\b.txt");
    }
}
