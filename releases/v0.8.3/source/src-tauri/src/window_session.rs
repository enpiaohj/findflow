//! 统一隐藏入口；原生标题栏、菜单和对话框的焦点变化不能结束搜索会话。
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::{Emitter, Manager};

static FOCUS_GENERATION: AtomicU64 = AtomicU64::new(0);

pub fn hide(app: &tauri::AppHandle, reason: &str) -> Result<(), String> {
    FOCUS_GENERATION.fetch_add(1, Ordering::SeqCst);
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().map_err(|e| e.to_string())? {
            let clear = app
                .try_state::<std::sync::Arc<crate::AppState>>()
                .map(|state| state.db.get_or("ui.clearOnHide", serde_json::json!(true)))
                .and_then(|value| value.as_bool())
                .unwrap_or(true);
            window.hide().map_err(|e| e.to_string())?;
            window
                .emit(
                    "findflow-hidden",
                    serde_json::json!({ "reason": reason, "clear": clear }),
                )
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn app_hide(app: tauri::AppHandle) -> Result<(), String> {
    hide(&app, "manual")
}

#[cfg(windows)]
fn app_is_foreground() -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
    unsafe {
        let hwnd = GetForegroundWindow();
        // 切换中的短暂空前台不视为真正离开，避免标题栏拖动误隐藏。
        if hwnd.0.is_null() {
            return true;
        }
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        pid == std::process::id()
    }
}

#[cfg(not(windows))]
fn app_is_foreground() -> bool {
    false
}

pub fn focus_changed(app: &tauri::AppHandle, focused: bool) {
    let generation = FOCUS_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    if focused {
        return;
    }
    let app = app.clone();
    // 丢弃句柄仅分离后台任务，不取消它；无需在窗口事件回调中阻塞等待。
    drop(tauri::async_runtime::spawn_blocking(move || {
        std::thread::sleep(std::time::Duration::from_millis(150));
        let target = app.clone();
        let _ = app.run_on_main_thread(move || {
            if FOCUS_GENERATION.load(Ordering::SeqCst) != generation || app_is_foreground() {
                return;
            }
            let Some(window) = target.get_webview_window("main") else {
                return;
            };
            if !window.is_visible().unwrap_or(false) || window.is_focused().unwrap_or(true) {
                return;
            }
            let Some(state) = target.try_state::<std::sync::Arc<crate::AppState>>() else {
                return;
            };
            if state
                .db
                .get_or("ui.blurHide", serde_json::json!(true))
                .as_bool()
                .unwrap_or(true)
            {
                let _ = hide(&target, "blur");
            }
        });
    }));
}
