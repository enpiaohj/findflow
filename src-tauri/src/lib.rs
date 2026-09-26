pub mod actions;
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
pub mod tokenizer;
pub mod updater_cmd;
pub mod util;
pub mod watcher;

use std::sync::{Arc, OnceLock};

use tauri::{Manager, WindowEvent};
use tauri_plugin_global_shortcut::GlobalShortcutExt;

pub struct HotkeyState(pub std::sync::Mutex<String>);

pub struct AppState {
    pub db: Arc<settings::SettingsStore>,
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
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        if let Some(w) = app.get_webview_window("main") {
                            if w.is_visible().unwrap_or(false) {
                                let _ = w.hide();
                            } else {
                                let _ = w.show();
                                let _ = w.set_focus();
                            }
                        }
                    }
                })
                .build(),
        )
        .setup(|app| {
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
                            for suffix in ["", "-wal", "-shm"] {
                                let from = default_dir.join(format!("findflow.db{}", suffix));
                                if from.exists() {
                                    let _ = std::fs::copy(&from, p.join(format!("findflow.db{}", suffix)));
                                }
                            }
                            tracing::info!("数据库已迁移至 {:?}", p);
                        }
                        p
                    }
                }
                Err(_) => default_dir.clone(),
            };
            let data_dir = data_dir;
            logger::init(&data_dir);

            let conn = db::open(&data_dir.join("findflow.db"))?;
            let store = Arc::new(settings::SettingsStore::new(conn));
            let index = searchindex::SearchIndex::open_or_create(&data_dir.join("index"))?;
            if index.fresh {
                // 索引重建：把全部文件重新入队，由 outbox 消费者从 SQLite 全量重放
                let reenqueued = store.with_conn(|c| {
                    c.execute(
                        "INSERT INTO index_outbox(file_id, operation, created_ms)
                         SELECT file_id, 'upsert', ?1 FROM files",
                        rusqlite::params![crate::util::now_ms()],
                    )
                    .ok();
                });
                let _ = reenqueued;
                tracing::info!("索引已重建，全量重放入队");
            }

            let engine = jobs::JobEngine::new(store.clone());
            engine.recover();

            let state = Arc::new(AppState {
                db: store.clone(),
                engine,
                index,
                watch: OnceLock::new(),
            });

            // outbox 消费 + 校准定时器
            let stop_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
            outbox::spawn_consumer(state.clone(), stop_flag.clone());
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

            app.manage(state);
            Ok(())
        })
        .on_window_event(|window, event| match event {
            WindowEvent::Focused(false) => {
                // 配置窗口先于 setup 创建，AppState 可能尚未注册
                let Some(state) = window.try_state::<std::sync::Arc<AppState>>() else {
                    return;
                };
                if window.label() == "main" {
                    let blur_hide = state.db.get_or("ui.blurHide", serde_json::json!(true));
                    if blur_hide.as_bool().unwrap_or(true) {
                        window.hide().ok();
                    }
                }
            }
            WindowEvent::CloseRequested { api, .. } => {
                // 两个窗口都只隐藏不销毁：管理窗销毁后齿轮将无法再次打开
                api.prevent_close();
                window.hide().ok();
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
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
            analysis::organize_execute,
            analysis::similar_images,
            actions::action_open,
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
            storage_cmd::set_data_dir,
            storage_cmd::relaunch_app,
        ])
        .run(tauri::generate_context!())
        .expect("FindFlow 启动失败");
}

pub mod storage_cmd {
    use serde::Serialize;
    use tauri::Manager;

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct StorageUsage {
        pub data_dir: String,
        pub db_bytes: u64,
        pub index_bytes: u64,
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

    #[tauri::command]
    pub fn storage_usage(
        _state: tauri::State<'_, crate::AppState>,
        app: tauri::AppHandle,
    ) -> Result<StorageUsage, String> {
        let data_dir = app
            .path()
            .app_data_dir()
            .map_err(|e| e.to_string())?;
        let marker = data_dir.join("storage-location.txt");
        let effective = std::fs::read_to_string(&marker)
            .map(|s| std::path::PathBuf::from(s.trim()))
            .unwrap_or(data_dir.clone());
        let db_bytes = std::fs::metadata(effective.join("findflow.db"))
            .map(|m| m.len())
            .unwrap_or(0);
        let index_bytes = crate::storage_cmd::dir_size(&effective.join("index"));
        Ok(StorageUsage {
            data_dir: effective.to_string_lossy().to_string(),
            db_bytes,
            index_bytes,
        })
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
