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
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            logger::init(&data_dir);

            let conn = db::open(&data_dir.join("findflow.db"))?;
            let store = Arc::new(settings::SettingsStore::new(conn));
            let index = searchindex::SearchIndex::open_or_create(&data_dir.join("index"))?;

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

            // 全局快捷键（默认 Alt+Space，可在设置中改键）
            let hotkey: String = state
                .db
                .get_or("ui.hotkey", serde_json::json!("Alt+Space"))
                .as_str()
                .unwrap_or("Alt+Space")
                .to_string();
            if let Ok(sc) = hotkey.parse::<tauri_plugin_global_shortcut::Shortcut>() {
                app.global_shortcut().register(sc).ok();
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
            diagnostics::diagnostics_health,
            diagnostics::diagnostics_failures,
            diagnostics::diagnostics_export,
            diagnostics::diagnostics_integrity,
            diagnostics::log_dir_get,
            updater_cmd::updater_check,
            updater_cmd::updater_install,
            window_cmds::open_manage,
        ])
        .run(tauri::generate_context!())
        .expect("FindFlow 启动失败");
}

pub mod window_cmds {
    use tauri::Manager;

    /// 打开完整管理窗（设置 / 分析 / 批量 / 任务）。
    #[tauri::command]
    pub fn open_manage(app: tauri::AppHandle) -> Result<(), String> {
        // 管理窗在 tauri.conf.json 中静态创建（启动时隐藏），这里只负责显示
        let w = app
            .get_webview_window("manage")
            .ok_or("管理窗尚未就绪，请稍候重试")?;
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
        Ok(())
    }
}
