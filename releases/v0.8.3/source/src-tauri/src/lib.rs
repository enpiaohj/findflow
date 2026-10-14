pub mod actions;
#[cfg(test)]
mod testutil;
pub mod analysis;
pub mod batchops;
pub mod db;
pub mod diagnostics;
pub mod dpapi;
pub mod extract;
pub mod identity;
pub mod jobs;
pub mod llm;
pub mod logger;
pub mod models;
pub mod nlq;
pub mod ocr;
pub mod outbox;
pub mod preview;
pub mod query;
pub mod scanner;
pub mod scope;
pub mod search;
pub mod searchindex;
pub mod settings;
pub mod shellicon;
pub mod tokenizer;
pub mod tray;
pub mod updater_cmd;
pub mod util;
pub mod watcher;
pub mod window_session;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use tauri::{Manager, WindowEvent};
use tauri_plugin_global_shortcut::GlobalShortcutExt;

/// 主窗是否已首次自动显示。窗口配置为 `visible: false`，等首屏页面加载完成后再显示，
/// 避免先显示空白 webview 造成白闪；开发热重载不应把用户已隐藏的窗口重新弹出，故只生效一次。
static MAIN_SHOWN: AtomicBool = AtomicBool::new(false);

/// 开机自启：带 `--ff-autostart` 参数静默启动到托盘，不弹出主窗。
fn is_autostart_launch() -> bool {
    std::env::args().any(|arg| arg == "--ff-autostart")
}

/// 首次显示主窗（显示 + 聚焦）；已显示过或开机自启时不做任何事。
fn show_main_once(app: &tauri::AppHandle) {
    // 静默开机启动同样初始化标题；以后从托盘呼出时无需重复初始化。
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.set_title(&tray::version_label(&app.package_info().version.to_string()));
    }
    if is_autostart_launch() || MAIN_SHOWN.swap(true, Ordering::SeqCst) {
        return;
    }
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
    }
}

pub struct HotkeyState(pub std::sync::Mutex<String>);

pub struct AppState {
    pub db: Arc<settings::SettingsStore>,
    pub bg_db: Arc<crate::settings::DbHandle>,
    /// 统计专用只读连接：诊断 / 范围计数等全表聚合走这里，不占用界面连接的互斥锁
    pub stats_db: Arc<crate::settings::DbHandle>,
    pub data_dir: std::path::PathBuf,
    pub engine: jobs::JobEngine,
    pub index: searchindex::SearchIndex,
    pub watch: OnceLock<watcher::WatchPool>,
}

impl AppState {
    pub fn watch_scope(&self, scope_id: i64, path: &str) -> Result<(), String> {
        if let Some(pool) = self.watch.get() {
            return pool.watch(scope_id, path);
        }
        Ok(())
    }

    pub fn unwatch_scope(&self, scope_id: i64) {
        if let Some(pool) = self.watch.get() {
            pool.unwatch(scope_id);
        }
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_single_instance::init(|app, _args, _cwd| {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        // 开机自启：带 --ff-autostart 参数静默启动到托盘，不弹出主窗
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--ff-autostart"]),
        ))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        if let Some(w) = app.get_webview_window("main") {
                            if w.is_visible().unwrap_or(false) {
                                let _ = window_session::hide(app, "manual");
                            } else {
                                let _ = w.show();
                                let _ = w.set_focus();
                            }
                        }
                    }
                })
                .build(),
        )
        // 首屏加载完成后再显示主窗，避免空白 webview 的白闪；
        // 开机自启（--ff-autostart）只驻留托盘，不自动弹出。
        .on_page_load(|webview, payload| {
            if webview.label() == "main"
                && payload.event() == tauri::webview::PageLoadEvent::Finished
            {
                show_main_once(webview.app_handle());
            }
        })
        .setup(|app| {
            // 窗口保持隐藏，等首屏加载完成后由 on_page_load 显示

            let default_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&default_dir)?;
            // 存储位置：默认目录下的 storage-location.txt 指向自定义目录；
            // 首次切换时自动迁移数据库文件（索引不迁移，重建即可）。
            let data_dir = match std::fs::read_to_string(default_dir.join("storage-location.txt")) {
                Ok(s) => {
                    let p = std::path::PathBuf::from(s.trim());
                    if p.as_os_str().is_empty() {
                        default_dir.clone()
                    } else {
                        let _ = std::fs::create_dir_all(&p);
                        if default_dir.join("findflow.db").exists() && !p.join("findflow.db").exists() {
                            let mut migrated = true;
                            for suffix in ["", "-wal", "-shm"] {
                                let from = default_dir.join(format!("findflow.db{}", suffix));
                                if from.exists()
                                    && std::fs::copy(&from, p.join(format!("findflow.db{}", suffix))).is_err()
                                {
                                    migrated = false;
                                }
                            }
                            if migrated {
                                // 迁移成功后删除旧库，避免旧数据目录长期残留（曾造成 GB 级冗余）。
                                // 索引与日志不迁移：索引在新位置重建，日志继续写新位置。
                                for suffix in ["", "-wal", "-shm"] {
                                    let _ = std::fs::remove_file(default_dir.join(format!("findflow.db{suffix}")));
                                }
                                let _ = std::fs::remove_dir_all(default_dir.join("index"));
                                tracing::info!("数据库已迁移至 {:?}，旧库文件已清理", p);
                            } else {
                                tracing::warn!("数据库迁移未完成，保留旧文件以便重试");
                            }
                        }
                        p
                    }
                }
                Err(_) => default_dir.clone(),
            };
            logger::init(&data_dir);
            // 扫描与监听跳过自身数据目录，防止范围包含数据目录时自我触发
            util::set_app_data_dir(&data_dir);

            // 界面专用连接 + 后台专用连接（WAL 读写并发互不阻塞）
            let conn = db::open(&data_dir.join("findflow.db"))?;
            let conn_bg = db::open(&data_dir.join("findflow.db"))?;
            let conn_stats = db::open(&data_dir.join("findflow.db"))?;
            let store = Arc::new(settings::SettingsStore::new(conn));
            let index = searchindex::SearchIndex::open_or_create(&data_dir.join("index"))?;
            if index.fresh {
                // 索引重建：后台分块入队（避免启动期长事务阻塞界面主线程）
                let bg_enqueue = Arc::new(crate::settings::DbHandle::new(db::open(
                    &data_dir.join("findflow.db"),
                )?));
                // 索引全量重放：旧的待处理事件已无意义，先清空避免重复处理
                bg_enqueue.with_conn(|c| c.execute("DELETE FROM index_outbox", []).ok());
                std::thread::Builder::new()
                    .name("reindex-enqueue".into())
                    .spawn(move || {
                        let mut last: i64 = 0;
                        loop {
                            let batch_max: i64 = bg_enqueue
                                .with_conn(|c| {
                                    c.query_row(
                                        "SELECT COALESCE(MAX(file_id), 0) FROM (
                                            SELECT file_id FROM files WHERE file_id > ?1
                                            ORDER BY file_id LIMIT 50000
                                        )",
                                        [last],
                                        |r| r.get::<_, i64>(0),
                                    )
                                    .map_err(|e| e.to_string())
                                })
                                .unwrap_or(0);
                            if batch_max <= last {
                                break;
                            }
                            let n = bg_enqueue.with_conn(|c| {
                                c.execute(
                                    "INSERT INTO index_outbox(file_id, operation, created_ms)
                                     SELECT file_id, 'upsert', ?3 FROM files
                                     WHERE file_id > ?1 AND file_id <= ?2",
                                    rusqlite::params![last, batch_max, crate::util::now_ms()],
                                )
                            })
                            .unwrap_or(0);
                            if n == 0 {
                                break;
                            }
                            last = batch_max;
                            std::thread::sleep(std::time::Duration::from_millis(60));
                        }
                        tracing::info!("索引重建入队完成");
                    })
                    .ok();
            }

            let bg_db = Arc::new(crate::settings::DbHandle::new(conn_bg));
            let engine = jobs::JobEngine::new(store.clone(), bg_db.clone());
            engine.recover();

            let state = Arc::new(AppState {
                db: store.clone(),
                bg_db,
                stats_db: Arc::new(crate::settings::DbHandle::new(conn_stats)),
                data_dir: data_dir.clone(),
                engine,
                index,
                watch: OnceLock::new(),
            });

            // outbox 消费 + 校准定时器
            let stop_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
            outbox::spawn_consumer(state.clone(), state.bg_db.clone(), stop_flag.clone());
            watcher::spawn_calibration_timer(state.clone(), stop_flag);

            // 恢复既有范围的监听
            let pool = watcher::WatchPool::new(state.clone());
            state.watch.set(pool).ok();
            let scopes: Vec<(i64, String)> = state.db.with_conn(|c| {
                let mut stmt = c
                    .prepare("SELECT scope_id, path FROM scopes WHERE enabled=1")
                    .unwrap();
                let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
                rows.filter_map(|x| x.ok()).collect()
            });
            for (sid, path) in scopes {
                let _ = state.watch_scope(sid, &path);
            }

            // 全局快捷键：用户设置优先；注册失败沿备用链降级（Alt+Space 与系统菜单键及
            // uTools/PowerToys 等普遍冲突），最终生效键回写设置供界面显示
            let user_hotkey: Option<String> = state
                .db
                .get("ui.hotkey")
                .ok()
                .flatten()
                .and_then(|v| v.as_str().map(|s| s.to_string()));
            let mut candidates: Vec<String> = Vec::new();
            if let Some(u) = &user_hotkey {
                candidates.push(u.clone());
            }
            candidates.push("Alt+Q".into());
            candidates.push("Ctrl+Alt+Space".into());
            candidates.push("Ctrl+Shift+Space".into());
            let gs = app.global_shortcut();
            let mut active: Option<String> = None;
            for candidate in &candidates {
                if let Ok(sc) = candidate.parse::<tauri_plugin_global_shortcut::Shortcut>() {
                    if gs.register(sc).is_ok() {
                        active = Some(candidate.clone());
                        break;
                    }
                    tracing::warn!("快捷键 {} 注册失败（可能被其他程序占用）", candidate);
                }
            }
            match &active {
                Some(active_key) => {
                    let _ = state.db.set("ui.hotkeyActive", &serde_json::json!(active_key));
                    if user_hotkey.is_none() {
                        let _ = state.db.set("ui.hotkey", &serde_json::json!(active_key));
                    }
                    app.manage(HotkeyState(std::sync::Mutex::new(active_key.clone())));
                }
                None => {
                    tracing::error!("所有备用快捷键均注册失败，应用仍可通过任务栏/直接运行使用");
                    app.manage(HotkeyState(std::sync::Mutex::new(String::new())));
                }
            }

            // 托盘是否显示由设置决定；必须在 manage 之前读取（state 随后被 move 进托管）
            let show_tray = state
                .db
                .get_or("ui.showTrayIcon", serde_json::json!(true))
                .as_bool()
                .unwrap_or(true);
            app.manage(state);

            // 系统托盘：左键呼出主窗，右键菜单提供「显示 FindFlow」「退出」——
            // 关闭按钮只隐藏窗口，退出由此入口承担
            tray::build(app.handle(), show_tray)?;

            // 自动定期校准：网络共享范围不做实时监听，只能靠校准保持同步；
            // 本地范围也借此兜住监听遗漏（休眠、快速改动等）。可在设置中关闭。
            scanner::start_auto_calibrate(app.handle().clone());

            // 自动正文抽取：默认关闭（待抽取量可达 27 万文件 / 54 GB，需数小时读盘），
            // 由用户在设置中开启后才在后台逐步建立正文索引。
            extract::start_auto_fulltext(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| match event {
            WindowEvent::Focused(focused) => {
                if window.label() == "main" {
                    window_session::focus_changed(window.app_handle(), *focused);
                }
            }
            WindowEvent::CloseRequested { api, .. } => {
                // 两个窗口都只隐藏不销毁：管理窗销毁后齿轮将无法再次打开
                api.prevent_close();
                if window.label() == "main" {
                    let _ = window_session::hide(window.app_handle(), "manual");
                } else {
                    window.hide().ok();
                }
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            window_session::app_hide,
            tray::tray_set_visible,
            tray::app_exit,
            settings::get_setting,
            settings::set_setting,
            settings::delete_setting,
            scope::scopes_list,
            scope::scope_add,
            scope::scope_remove,
            scope::scope_set_enabled,
            scope::scope_update_options,
            scope::scope_trial_scan,
            scanner::cmd_scan,
            scanner::cmd_recalibrate,
            scanner::cmd_rebuild,
            search::cmd_search,
            search::cmd_search_count,
            nlq::cmd_nl_parse,
            llm::llm_has_key,
            llm::llm_set_key,
            llm::llm_clear_key,
            llm::nl_llm_parse,
            extract::cmd_fulltext_start,
            ocr::cmd_ocr_start,
            analysis::large_files,
            analysis::start_duplicates_cmd,
            analysis::organize_suggest,
            analysis::organize_issue_token,
            analysis::organize_execute,
            analysis::similar_images,
            actions::action_open,
            actions::path_probe,
            actions::action_locate,
            actions::action_copy_path,
            actions::action_copy_files,
            actions::action_recycle,
            actions::action_delete_permanent,
            actions::action_issue_delete_token,
            actions::action_open_with,
            actions::action_properties,
            actions::action_open_terminal,
            actions::action_open_vscode,
            actions::action_rename,
            batchops::batch_rename_preview,
            batchops::batch_rename_execute,
            batchops::batch_rename_undo,
            batchops::batch_move_preview,
            batchops::batch_move_execute,
            preview::preview_get,
            preview::favorites_list,
            preview::favorite_toggle,
            preview::tags_list,
            preview::tag_create,
            preview::tag_delete,
            preview::file_tags_set,
            preview::recents_list,
            preview::file_by_path,
            shellicon::file_icons,
            shellicon::file_thumbnail,
            jobs::jobs_list,
            jobs::job_pause,
            jobs::job_resume,
            jobs::job_cancel,
            jobs::job_retry_failed,
            jobs::scan_status,
            diagnostics::diagnostics_health,
            diagnostics::diagnostics_failures,
            diagnostics::diagnostics_export,
            diagnostics::diagnostics_integrity,
            diagnostics::log_dir_get,
            updater_cmd::updater_check,
            updater_cmd::updater_install,
            hotkey::hotkey_apply,
            storage_cmd::storage_usage,
            storage_cmd::cleanup_legacy_data,
            storage_cmd::set_data_dir,
            storage_cmd::relaunch_app,
        ])
        .run(tauri::generate_context!())
        .expect("FindFlow 启动失败");
}

pub mod storage_cmd {
    use serde::Serialize;
    use tauri::Manager;

    #[derive(Serialize, Clone)]
    #[serde(rename_all = "camelCase")]
    pub struct StorageUsage {
        pub data_dir: String,
        pub db_bytes: u64,
        pub index_bytes: u64,
        /// 旧数据目录（默认数据目录 ≠ 当前数据目录时存在），用于提示清理。
        pub legacy_dir: Option<String>,
        /// 旧数据目录中可清理的冗余占用（数据库 + 索引，不含 llm.key 与日志）。
        pub legacy_bytes: u64,
    }

    fn dir_size(p: &std::path::Path) -> u64 {
        let mut sum = 0u64;
        if let Ok(rd) = std::fs::read_dir(p) {
            for e in rd.flatten() {
                if e.path().is_dir() {
                    sum += dir_size(&e.path());
                } else if let Ok(m) = e.metadata() {
                    sum += m.len();
                }
            }
        }
        sum
    }

    /// 旧数据目录中可安全清理的占用：数据库三件套 + 索引目录。
    /// 不含 `llm.key`（当前生效的密钥）、`logs/` 与 `storage-location.txt`。
    fn legacy_cleanable_bytes(dir: &std::path::Path) -> u64 {
        let mut sum = 0u64;
        for name in ["findflow.db", "findflow.db-wal", "findflow.db-shm"] {
            sum += std::fs::metadata(dir.join(name)).map(|m| m.len()).unwrap_or(0);
        }
        sum + dir_size(&dir.join("index"))
    }

    /// 默认数据目录：当它不同于当前生效目录时，其中的数据库与索引即旧数据残留。
    fn legacy_dir_of(app: &tauri::AppHandle, data_dir: &std::path::Path) -> Option<std::path::PathBuf> {
        let default_dir = app.path().app_data_dir().ok()?;
        let same = dunce::simplified(&default_dir) == dunce::simplified(data_dir);
        if same || !default_dir.join("findflow.db").exists() {
            return None;
        }
        Some(default_dir)
    }

    /// 当前生效数据目录的占用（数据库含 WAL + 索引目录），并报告旧数据目录残留。
    #[tauri::command]
    pub async fn storage_usage(
        app: tauri::AppHandle,
        state: tauri::State<'_, std::sync::Arc<crate::AppState>>,
    ) -> Result<StorageUsage, String> {
        let dir = state.data_dir.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let file_len = |name: &str| {
                std::fs::metadata(dir.join(name))
                    .map(|m| m.len())
                    .unwrap_or(0)
            };
            let legacy = legacy_dir_of(&app, &dir);
            let legacy_bytes = legacy.as_ref().map(|d| legacy_cleanable_bytes(d)).unwrap_or(0);
            StorageUsage {
                data_dir: dir.to_string_lossy().to_string(),
                db_bytes: file_len("findflow.db") + file_len("findflow.db-wal"),
                index_bytes: dir_size(&dir.join("index")),
                legacy_dir: legacy.map(|d| d.to_string_lossy().to_string()),
                legacy_bytes,
            }
        })
        .await
        .map_err(|e| e.to_string())
    }

    /// 清理旧数据目录中的冗余（数据库三件套 + 索引），保留 llm.key、日志与指针文件。
    /// 仅清理「默认数据目录 ≠ 当前生效目录」时的残留；当前生效目录永不被清理。
    #[tauri::command]
    pub async fn cleanup_legacy_data(
        app: tauri::AppHandle,
        state: tauri::State<'_, std::sync::Arc<crate::AppState>>,
    ) -> Result<u64, String> {
        let dir = state.data_dir.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let Some(legacy) = legacy_dir_of(&app, &dir) else {
                return Ok(0u64);
            };
            let freed = legacy_cleanable_bytes(&legacy);
            for name in ["findflow.db", "findflow.db-wal", "findflow.db-shm"] {
                let f = legacy.join(name);
                if f.exists() {
                    std::fs::remove_file(&f).map_err(|e| format!("删除 {name} 失败：{e}"))?;
                }
            }
            let idx = legacy.join("index");
            if idx.exists() {
                std::fs::remove_dir_all(&idx).map_err(|e| format!("删除旧索引失败：{e}"))?;
            }
            tracing::info!("已清理旧数据目录残留：{:?}（释放 {} 字节）", legacy, freed);
            Ok(freed)
        })
        .await
        .map_err(|e| e.to_string())?
    }

    /// 更改存储位置：写入指针文件，重启后自动迁移数据库并重建索引。
    #[tauri::command]
    pub fn set_data_dir(
        _state: tauri::State<'_, std::sync::Arc<crate::AppState>>,
        app: tauri::AppHandle,
        new_dir: String,
    ) -> Result<(), String> {
        let p = std::path::PathBuf::from(new_dir.trim());
        if p.as_os_str().is_empty() {
            return Err("路径不能为空".into());
        }
        std::fs::create_dir_all(&p).map_err(|e| format!("无法创建目录：{e}"))?;
        if crate::scope::is_protected(&crate::util::normalize_path(&p)) {
            return Err("系统与程序目录受保护，不能作为存储位置".into());
        }
        let marker = app
            .path()
            .app_data_dir()
            .map_err(|e| e.to_string())?
            .join("storage-location.txt");
        std::fs::write(&marker, p.to_string_lossy().as_bytes())
            .map_err(|e| format!("写入失败：{e}"))?;
        Ok(())
    }

    /// 重启应用（应用存储位置迁移后调用）。
    #[tauri::command]
    pub fn relaunch_app(app: tauri::AppHandle) -> Result<(), String> {
        app.restart();
        #[allow(unreachable_code)]
        Ok(())
    }
}

pub mod hotkey {
    use tauri::Manager;
    use tauri_plugin_global_shortcut::GlobalShortcutExt;

    /// 改键即时生效：解绑旧键、注册新键；失败自动恢复旧键（FR01 异常闭环）。
    #[tauri::command]
    pub fn hotkey_apply(
        app: tauri::AppHandle,
        state: tauri::State<'_, crate::HotkeyState>,
        new_hotkey: String,
    ) -> Result<(), String> {
        use tauri_plugin_global_shortcut::Shortcut;
        let new_sc: Shortcut = new_hotkey
            .parse()
            .map_err(|_| "快捷键格式无法识别，例如 Alt+Space 或 Ctrl+J".to_string())?;
        let gs = app.global_shortcut();
        let old = state.0.lock().unwrap().clone();
        if let Ok(old_sc) = old.parse::<Shortcut>() {
            let _ = gs.unregister(old_sc);
        }
        match gs.register(new_sc) {
            Ok(()) => {
                *state.0.lock().unwrap() = new_hotkey.clone();
                crate::settings::persist_hotkey(&app, &new_hotkey);
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.eval("window.dispatchEvent(new CustomEvent('hotkey-changed'))");
                }
                Ok(())
            }
            Err(e) => {
                if let Ok(old_sc) = old.parse::<Shortcut>() {
                    let _ = gs.register(old_sc);
                }
                Err(format!("注册失败（可能与其它应用冲突）：{e}"))
            }
        }
    }
}
