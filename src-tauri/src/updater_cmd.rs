use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::Emitter;
use tauri_plugin_updater::UpdaterExt;

/// 下载进度事件名（前端「关于」页监听）。
pub const PROGRESS_EVENT: &str = "updater-progress";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    pub available: bool,
    /// 可用的新版本号；没有更新时为 None
    pub version: Option<String>,
    /// 更新说明（取自更新清单的 notes）
    pub notes: Option<String>,
    /// 当前运行的版本号
    pub current_version: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct UpdateProgress {
    downloaded: u64,
    /// 服务器未给出总大小时为 None（界面显示不定进度）
    total: Option<u64>,
    /// 下载完成、即将启动安装程序
    finished: bool,
}

/// 检查更新。只在用户主动点击时调用：这是除可选的自然语言搜索外唯一的联网请求，
/// 只读取 GitHub Releases 上的更新清单，不发送任何本机数据。
#[tauri::command]
pub async fn updater_check(app: tauri::AppHandle) -> Result<UpdateStatus, String> {
    let current_version = app.package_info().version.to_string();
    let update = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| format!("检查更新失败：{e}"))?;
    Ok(match update {
        Some(u) => UpdateStatus {
            available: true,
            version: Some(u.version.clone()),
            notes: u.body.clone(),
            current_version,
        },
        None => UpdateStatus {
            available: false,
            version: None,
            notes: None,
            current_version,
        },
    })
}

/// 下载并安装（校验签名）。下载过程通过 [`PROGRESS_EVENT`] 汇报进度；
/// 安装程序启动后本进程会退出，由安装程序在完成后重新启动应用（FR19）。
#[tauri::command]
pub async fn updater_install(app: tauri::AppHandle) -> Result<(), String> {
    let update = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?
        .ok_or("当前没有可用更新")?;

    let downloaded = AtomicU64::new(0);
    // 进度事件节流：每 100ms 至多一条，避免大文件下载时刷屏
    let last_emit = Mutex::new(Instant::now() - Duration::from_secs(1));
    let total_seen = Mutex::new(None::<u64>);
    let progress_app = app.clone();
    let finish_app = app.clone();

    update
        .download_and_install(
            |chunk: usize, total: Option<u64>| {
                let done = downloaded.fetch_add(chunk as u64, Ordering::Relaxed) + chunk as u64;
                *total_seen.lock().unwrap() = total;
                let mut last = last_emit.lock().unwrap();
                if last.elapsed() >= Duration::from_millis(100) || Some(done) == total {
                    *last = Instant::now();
                    let _ = progress_app.emit(
                        PROGRESS_EVENT,
                        UpdateProgress { downloaded: done, total, finished: false },
                    );
                }
            },
            || {
                let _ = finish_app.emit(
                    PROGRESS_EVENT,
                    UpdateProgress {
                        downloaded: downloaded.load(Ordering::Relaxed),
                        total: *total_seen.lock().unwrap(),
                        finished: true,
                    },
                );
            },
        )
        .await
        .map_err(|e| format!("下载或安装失败：{e}"))?;
    Ok(())
}
