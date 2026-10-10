//! 系统托盘：左键呼出主窗，右键菜单提供「显示 FindFlow」「退出」。
//! 关闭按钮只隐藏窗口，退出由此入口承担；托盘是否显示由设置 ui.showTrayIcon 决定。

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

/// 创建托盘图标；`visible` 为 false 时创建后立即隐藏（对应设置项 ui.showTrayIcon）。
pub fn build(app: &AppHandle, visible: bool) -> tauri::Result<()> {
    let show_item = MenuItem::with_id(app, "show", "显示 FindFlow", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let tray_menu = Menu::with_items(app, &[&show_item, &quit_item])?;

    let tray = TrayIconBuilder::with_id("main-tray")
        .icon(app.default_window_icon().expect("应用图标已配置").clone())
        .tooltip("FindFlow")
        .menu(&tray_menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        // 左键呼出主窗（不做显隐切换，避免与失焦隐藏竞争）
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }
        })
        .build(app)?;

    if !visible {
        let _ = tray.set_visible(false);
    }
    Ok(())
}

/// 显示或隐藏系统托盘图标（设置项 ui.showTrayIcon 的运行时生效入口）。
#[tauri::command]
pub fn tray_set_visible(app: AppHandle, visible: bool) -> Result<(), String> {
    if let Some(tray) = app.tray_by_id("main-tray") {
        tray.set_visible(visible).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// 完全退出应用：关闭窗口只隐藏到后台，退出由本命令承担。
#[tauri::command]
pub fn app_exit(app: AppHandle) {
    app.exit(0);
}
