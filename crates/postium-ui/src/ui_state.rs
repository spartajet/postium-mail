//! UI 偏好持久化（替代旧版 localStorage）：~/.postium/ui-state.json

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default)]
pub struct UiState {
    pub theme: Option<String>, // "dark" | "light"
    pub lang: Option<String>,  // "zh" | "en"
}

fn path() -> std::path::PathBuf {
    crate::resolve_data_dir().join("ui-state.json")
}

pub fn load() -> UiState {
    std::fs::read_to_string(path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(state: &UiState) {
    if let Ok(json) = serde_json::to_string_pretty(state) {
        let _ = std::fs::write(path(), json);
    }
}
