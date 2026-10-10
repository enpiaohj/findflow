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
        "doc" | "docx" | "docm" | "dot" | "dotx" | "odt" | "wps" | "pages" | "txt" | "md" | "rtf"
        | "tex" | "epub" | "mht" | "mhtml" | "vsd" | "vsdx" | "html" | "htm" | "log" => "doc",
        "xls" | "xlsx" | "xlsm" | "xlsb" | "et" | "numbers" | "csv" | "tsv" | "ods" => "sheet",
        "pdf" => "pdf",
        "png" | "jpg" | "jpeg" | "jfif" | "jpe" | "jp2" | "gif" | "bmp" | "webp" | "svg" | "ico"
        | "tif" | "tiff" | "heic" | "heif" | "avif" | "psd" | "cr2" | "nef" | "arw" | "dng" => {
            "image"
        }
        // 视频：不含 `.ts` / `.mts`——这两个扩展名在开发机上绝大多数是 TypeScript
        // 源码与声明文件（`.d.mts`），归为视频会大面积误分类（实测本机 2825 个 .mts）。
        // MPEG 传输流用 `.m2ts` 覆盖。
        "mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "webm" | "m4v" | "mpg" | "mpeg"
        | "rmvb" | "rm" | "m2ts" | "m2v" | "3gp" | "3g2" | "vob" | "asf" | "f4v" | "divx" | "ogv"
        | "mxf" => "video",
        "mp3" | "wav" | "flac" | "aac" | "ogg" | "oga" | "m4a" | "m4b" | "wma" | "ape" | "opus"
        | "aiff" | "aif" | "amr" | "mid" | "midi" | "ac3" | "dts" | "mka" | "caf" | "weba"
        | "ra" => "audio",
        "zip" | "rar" | "7z" | "tar" | "tgz" | "txz" | "gz" | "bz2" | "xz" | "zst" | "lz4"
        | "lz" | "br" | "cab" | "iso" | "arj" | "lzh" => "archive",
        _ => "other",
    }
}

/// 归一化为不带 `\\?\` 前缀的常规路径，供显示与数据库存储。
pub fn normalize_path(p: &Path) -> String {
    dunce::simplified(p).to_string_lossy().replace('/', "\\")
}

/// 把可能含 8.3 短名（如 `PROGRA~1`）的路径解析为长名形式，供保护目录判断使用。
/// 短名可绕过按段比对的 `is_protected`（`C:\PROGRA~1` 不等于 `Program Files`），
/// 因此安全判定前必须先展开。解析失败（路径不存在、无权限、已是长名）时原样返回。
pub fn to_long_path(p: &str) -> String {
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::GetLongPathNameW;

    if p.is_empty() {
        return p.to_string();
    }
    let wide: Vec<u16> = p.encode_utf16().chain(std::iter::once(0)).collect();
    // 先问所需长度；返回 0 表示解析失败，直接用原值。
    let need = unsafe { GetLongPathNameW(PCWSTR(wide.as_ptr()), None) };
    if need == 0 {
        return p.to_string();
    }
    let mut buf = vec![0u16; need as usize];
    let written = unsafe { GetLongPathNameW(PCWSTR(wide.as_ptr()), Some(&mut buf)) };
    if written == 0 {
        return p.to_string();
    }
    buf.truncate(written as usize);
    String::from_utf16_lossy(&buf).replace('/', "\\")
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

/// 开发目录 / 构建产物目录名（小写）。这些目录里几乎只有第三方依赖与编译产物，
/// 不是用户文档，正文抽取跳过它们，避免把 node_modules 里的源码当文档索引。
/// 名称索引不受影响——用户仍能按文件名搜到这些目录里的文件。
const DEV_DIRS: &[&str] = &[
    "node_modules",
    ".git",
    ".svn",
    ".hg",
    "__pycache__",
    ".venv",
    "venv",
    ".tox",
    ".next",
    ".nuxt",
    ".cache",
    ".gradle",
    ".idea",
    ".vs",
    "target",
    "dist",
    "build",
    "coverage",
];

/// 路径是否经过某个开发 / 构建目录（按完整路径段比对，避免误伤 `node_modules_notes` 这类名字）。
pub fn is_dev_dir_path(path: &str) -> bool {
    path.to_lowercase()
        .split(['\\', '/'])
        .any(|seg| DEV_DIRS.contains(&seg))
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
    fn audio_video_cover_common_extensions() {
        // 「音乐」「视频」筛选要覆盖各来源的常见容器格式，缺一个就会漏文件
        for e in [
            "mp3", "wav", "flac", "aac", "ogg", "oga", "m4a", "wma", "ape", "opus", "aiff",
            "aif", "m4b", "amr", "mid", "midi", "ac3", "dts", "mka", "caf", "weba", "ra",
        ] {
            assert_eq!(kind_from(e, false), "audio", "{e} 应归为 audio");
        }
        for e in [
            "mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v", "mpg", "mpeg", "rmvb",
            "rm", "m2ts", "m2v", "3gp", "3g2", "vob", "asf", "f4v", "divx", "ogv", "mxf",
        ] {
            assert_eq!(kind_from(e, false), "video", "{e} 应归为 video");
        }
        // `.ts` / `.mts` 不归视频：这两个扩展名绝大多数是 TypeScript 源码与声明文件
        // （本机实测 .mts 有 2825 个，几乎全是 node_modules 里的 .d.mts）
        assert_eq!(kind_from("ts", false), "other");
        assert_eq!(kind_from("mts", false), "other");
        // 大小写不敏感（扫描器传入的 ext 已小写，此处保证直接调用也正确）
        assert_eq!(kind_from("MP3", false), "audio");
        assert_eq!(kind_from("MKV", false), "video");
    }

    #[test]
    fn document_kinds_cover_office_formats() {
        // 「文档」筛选依赖这些扩展名归入 doc / sheet / pdf；
        // 漏掉 xlsm / odt / wps 等会让对应文件在筛选时消失
        for e in [
            "doc", "docx", "docm", "dot", "dotx", "odt", "wps", "pages", "txt", "md", "rtf",
            "tex", "epub", "mht", "mhtml", "vsd", "vsdx", "html", "htm", "log",
        ] {
            assert_eq!(kind_from(e, false), "doc", "{e} 应归为 doc");
        }
        for e in ["xls", "xlsx", "xlsm", "xlsb", "et", "numbers", "csv", "tsv", "ods"] {
            assert_eq!(kind_from(e, false), "sheet", "{e} 应归为 sheet");
        }
        assert_eq!(kind_from("pdf", false), "pdf");
        // 新增的图片 / 压缩包格式
        for e in ["heic", "heif", "avif", "jfif", "jpe", "jp2", "psd", "cr2", "nef", "arw", "dng"] {
            assert_eq!(kind_from(e, false), "image", "{e} 应归为 image");
        }
        for e in [
            "zip", "rar", "7z", "tar", "tgz", "txz", "gz", "bz2", "xz", "zst", "lz4", "lz",
            "br", "cab", "iso", "arj", "lzh",
        ] {
            assert_eq!(kind_from(e, false), "archive", "{e} 应归为 archive");
        }
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
    fn dev_dir_detection_matches_path_segments_only() {
        // 命中开发 / 构建目录
        assert!(is_dev_dir_path(r"D:\p\node_modules\lucide-react\a.js"));
        assert!(is_dev_dir_path(r"D:\p\.git\objects\x"));
        assert!(is_dev_dir_path(r"D:\p\src-tauri\target\debug\x.pdb"));
        assert!(is_dev_dir_path("D:/p/dist/index.js"));
        assert!(is_dev_dir_path(r"D:\p\__pycache__\a.pyc"));
        assert!(is_dev_dir_path(r"D:\NODE_MODULES\a.js"), "大小写不敏感");
        // 只是名字里含关键词的普通目录不受影响（按完整段比对）
        assert!(!is_dev_dir_path(r"D:\p\node_modules_notes\a.txt"));
        assert!(!is_dev_dir_path(r"D:\p\targets\a.txt"));
        assert!(!is_dev_dir_path(r"D:\我的文档\dist报告\a.docx"));
        assert!(!is_dev_dir_path(r"D:\p\src\a.rs"));
    }

    #[test]
    fn normalize_strips_verbatim() {
        assert_eq!(normalize_path(Path::new("D:\\\\a\\\\b.txt")), "D:\\\\a\\\\b.txt");
    }
}
