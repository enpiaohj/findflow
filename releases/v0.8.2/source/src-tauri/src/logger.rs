use std::path::Path;
use std::sync::OnceLock;

static LOG_DIR: OnceLock<std::path::PathBuf> = OnceLock::new();

/// 初始化滚动日志；日志对用户目录名做缩短处理，搜索词不落盘。
pub fn init(data_dir: &Path) {
    let dir = data_dir.join("logs");
    let _ = std::fs::create_dir_all(&dir);
    let _ = LOG_DIR.set(dir.clone());
    let appender = tracing_appender::rolling::daily(&dir, "findflow.log");
    // tantivy 每次提交/合并都输出多行 INFO，会淹没应用日志并撑大日志文件，只保留其警告
    let filter = tracing_subscriber::EnvFilter::new("info,tantivy=warn");
    tracing_subscriber::fmt()
        .with_writer(appender)
        .with_ansi(false)
        .with_env_filter(filter)
        .init();
    // 后台清理：诊断日志默认保留 14 天（设计文档 §8.3）。
    std::thread::spawn(|| {
        if let Some(dir) = LOG_DIR.get() {
            let cutoff = std::time::SystemTime::now() - std::time::Duration::from_secs(14 * 86400);
            if let Ok(entries) = std::fs::read_dir(dir) {
                for e in entries.flatten() {
                    if let Ok(meta) = e.metadata() {
                        if let Ok(modified) = meta.modified() {
                            if modified < cutoff {
                                let _ = std::fs::remove_file(e.path());
                            }
                        }
                    }
                }
            }
        }
    });
}

pub fn log_dir() -> Option<&'static Path> {
    LOG_DIR.get().map(|p| p.as_path())
}
