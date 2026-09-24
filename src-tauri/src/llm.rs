use std::path::PathBuf;

use serde::Deserialize;
use tauri::Manager;

use crate::models::QueryAst;

const API_URL: &str = "https://api.deepseek.com/chat/completions";

fn key_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|_| "无法定位应用数据目录".to_string())?;
    Ok(dir.join("llm.key"))
}

fn read_key(app: &tauri::AppHandle) -> Result<String, String> {
    let path = key_path(app)?;
    let blob = std::fs::read(&path).map_err(|_| "未配置 API Key".to_string())?;
    let plain = crate::dpapi::unprotect(&blob)?;
    String::from_utf8(plain).map_err(|_| "Key 解码失败".into())
}

#[tauri::command]
pub fn llm_has_key(app: tauri::AppHandle) -> Result<bool, String> {
    Ok(key_path(&app).map(|p| p.exists()).unwrap_or(false))
}

/// 运行时保存 Key：DPAPI CurrentUser 加密后落盘（§10.3）。
#[tauri::command]
pub fn llm_set_key(app: tauri::AppHandle, key: String) -> Result<(), String> {
    let path = key_path(&app)?;
    let blob = crate::dpapi::protect(key.trim().as_bytes())?;
    std::fs::write(&path, blob).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn llm_clear_key(app: tauri::AppHandle) -> Result<(), String> {
    let path = key_path(&app)?;
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Deserialize)]
struct ChatMessage {
    content: String,
}

/// 可选在线 LLM：只把查询文本与字段定义发给 DeepSeek，返回查询 AST，
/// LLM 不访问磁盘、不执行文件操作（§10.3、FR16）。
#[tauri::command]
pub async fn nl_llm_parse(
    app: tauri::AppHandle,
    text: String,
) -> Result<QueryAst, String> {
    let key = tauri::async_runtime::spawn_blocking(move || read_key(&app))
        .await
        .map_err(|e| e.to_string())??;
    let system = "你是 Windows 文件搜索查询解析器。把用户的中文自然语言转换为 JSON 查询条件，\
字段定义：text(关键词字符串), phrase(精确短语), exts(扩展名数组,不带点), kinds(类型数组: doc/sheet/pdf/image/video/audio/archive/folder), \
pathInclude(路径包含数组), pathExclude, sizeMin(字节数), sizeMax, modifiedAfterMs(毫秒时间戳), modifiedBeforeMs。\
只输出 JSON 对象本身，不要解释。无法识别的部分留空。当前时间戳毫秒："
        .to_string()
        + &crate::util::now_ms().to_string();

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .post(API_URL)
        .bearer_auth(key.trim())
        .json(&serde_json::json!({
            "model": "deepseek-chat",
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": text}
            ],
            "response_format": {"type": "json_object"},
            "temperature": 0
        }))
        .send()
        .await
        .map_err(|e| format!("请求失败：{e}"))?;
    if !resp.status().is_success() {
        let code = resp.status();
        let body = resp.text().await.unwrap_or_default();
        // 不回显完整响应，避免泄漏 Key 相关信息
        let hint = if body.contains("auth") || code == reqwest::StatusCode::UNAUTHORIZED {
            "认证失败，请检查 API Key"
        } else {
            "服务返回错误"
        };
        return Err(format!("DeepSeek {code}: {hint}"));
    }
    let parsed: ChatResponse = resp.json().await.map_err(|e| e.to_string())?;
    let content = parsed
        .choices
        .first()
        .map(|c| c.message.content.clone())
        .ok_or("空响应")?;
    let ast: QueryAst = serde_json::from_str(&content)
        .map_err(|e| format!("AST 解析失败（原始内容已忽略）：{e}"))?;
    Ok(ast)
}
