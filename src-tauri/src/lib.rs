mod db;
mod settings;

use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let conn = db::open(&data_dir.join("findflow.db"))?;
            app.manage(settings::SettingsStore::new(conn));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            settings::get_setting,
            settings::set_setting,
            settings::delete_setting
        ])
        .run(tauri::generate_context!())
        .expect("FindFlow 启动失败");
}
