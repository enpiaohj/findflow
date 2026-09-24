use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileRow {
    pub file_id: i64,
    pub name: String,
    pub path: String,
    pub ext: String,
    pub kind: String,
    pub size: i64,
    pub modified_ms: i64,
    pub created_ms: i64,
    pub is_dir: bool,
    #[serde(default)]
    pub fulltext_state: String,
    #[serde(default)]
    pub snippet: Option<String>,
    #[serde(default)]
    pub score: f64,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopeInfo {
    pub scope_id: i64,
    pub path: String,
    pub enabled: bool,
    pub state: String,
    pub files_count: i64,
    pub baseline_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrialScanResult {
    pub files_est: i64,
    pub bytes_est: i64,
    pub sample: Vec<String>,
    pub accessible: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SortMode {
    #[default]
    Relevance,
    Name,
    Modified,
    Size,
}

/// 统一查询 AST：关键词、筛选语法与自然语言共同的目标形式（设计文档 §9、FR16）。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct QueryAst {
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub phrase: Option<String>,
    #[serde(default)]
    pub exts: Vec<String>,
    #[serde(default)]
    pub kinds: Vec<String>,
    #[serde(default)]
    pub path_include: Vec<String>,
    #[serde(default)]
    pub path_exclude: Vec<String>,
    #[serde(default)]
    pub text_exclude: Vec<String>,
    #[serde(default)]
    pub size_min: Option<i64>,
    #[serde(default)]
    pub size_max: Option<i64>,
    #[serde(default)]
    pub modified_after_ms: Option<i64>,
    #[serde(default)]
    pub modified_before_ms: Option<i64>,
    #[serde(default)]
    pub sort: SortMode,
}

impl QueryAst {
    pub fn has_filters(&self) -> bool {
        !self.exts.is_empty()
            || !self.kinds.is_empty()
            || !self.path_include.is_empty()
            || !self.path_exclude.is_empty()
            || !self.text_exclude.is_empty()
            || self.size_min.is_some()
            || self.size_max.is_some()
            || self.modified_after_ms.is_some()
            || self.modified_before_ms.is_some()
    }

    /// 供 UI 展示的人类可读解释。
    pub fn explain(&self) -> Vec<String> {
        let mut parts = Vec::new();
        if let Some(t) = &self.text {
            parts.push(format!("关键词「{t}」"));
        }
        if let Some(p) = &self.phrase {
            parts.push(format!("短语「{p}」"));
        }
        if !self.exts.is_empty() {
            parts.push(format!("扩展名 {}", self.exts.join("/")));
        }
        if !self.kinds.is_empty() {
            parts.push(format!("类型 {}", self.kinds.join("/")));
        }
        if !self.path_include.is_empty() {
            parts.push(format!("路径包含 {}", self.path_include.join("/")));
        }
        for p in &self.path_exclude {
            parts.push(format!("排除路径「{p}」"));
        }
        for t in &self.text_exclude {
            parts.push(format!("排除关键词「{t}」"));
        }
        if let Some(s) = self.size_min {
            parts.push(format!("大小 ≥ {}", format_size(s)));
        }
        if let Some(s) = self.size_max {
            parts.push(format!("大小 ≤ {}", format_size(s)));
        }
        if let Some(m) = self.modified_after_ms {
            let dt = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(m)
                .map(|d| d.format("%Y-%m-%d").to_string())
                .unwrap_or_default();
            parts.push(format!("修改时间 ≥ {dt}"));
        }
        if let Some(m) = self.modified_before_ms {
            let dt = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(m)
                .map(|d| d.format("%Y-%m-%d").to_string())
                .unwrap_or_default();
            parts.push(format!("修改时间 ≤ {dt}"));
        }
        parts
    }
}

pub fn format_size(bytes: i64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    const TB: f64 = GB * 1024.0;
    let b = bytes as f64;
    if b >= TB {
        format!("{:.1} TB", b / TB)
    } else if b >= GB {
        format!("{:.1} GB", b / GB)
    } else if b >= MB {
        format!("{:.1} MB", b / MB)
    } else if b >= KB {
        format!("{:.0} KB", b / KB)
    } else {
        format!("{bytes} B")
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobInfo {
    pub job_id: i64,
    pub kind: String,
    pub state: String,
    pub progress: f64,
    pub message: Option<String>,
    pub result_json: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroup {
    pub hash: String,
    pub files: Vec<FileRow>,
    pub wasted: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizerRule {
    pub rule_id: i64,
    pub name: String,
    /// extension | date_ym | pattern | size_gt
    pub match_type: String,
    pub match_value: String,
    pub target_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanItem {
    pub file_id: i64,
    pub old_path: String,
    pub new_path: String,
    pub status: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexHealth {
    pub total_files: i64,
    pub fulltext_done: i64,
    pub fulltext_failed: i64,
    pub ocr_done: i64,
    pub ocr_failed: i64,
    pub outbox_pending: i64,
    pub db_size_bytes: i64,
    pub scopes: Vec<ScopeInfo>,
    pub last_checkpoint_ms: i64,
}
