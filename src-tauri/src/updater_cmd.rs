use serde::Serialize;

use tauri_plugin_updater::UpdaterExt;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    pub available: bool,
    pub version: Option<String>,
    pub notes: Option<String>,
}

#[tauri::command]
pub async fn updater_check(app: tauri::AppHandle) -> Result<UpdateStatus, String> {
    let update = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| format!("检查更新失败：{e}"))?;
    match update {
        Some(u) => Ok(UpdateStatus {
            available: true,
            version: Some(u.version.clone()),
            notes: u.body.clone(),
        }),
        None => Ok(UpdateStatus {
            available: false,
            version: None,
            notes: None,
        }),
    }
}

/// 下载并安装（校验签名），安装后应用重启（FR19）。
#[tauri::command]
pub async fn updater_install(app: tauri::AppHandle) -> Result<(), String> {
    let update = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?
        .ok_or("当前没有可用更新")?;
    update
        .download_and_install(
            &mut |_chunk: usize, _total: Option<u64>| {},
            || {},
        )
        .await
        .map_err(|e| format!("下载或安装失败：{e}"))?;
    Ok(())
}
