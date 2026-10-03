use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontPreset {
    pub id: String,
    pub label: String,
    pub preview_family: String,
    pub available: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapPayload {
    pub presets: Vec<FontPreset>,
    pub active_preset_id: Option<String>,
    pub active_font_label: String,
    pub current_preview_family: String,
    pub backup_dir: String,
    pub can_restore: bool,
    pub pending_recovery: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ActionResult {
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "camelCase")]
pub enum Operation {
    Apply {
        preset_id: String,
        paths: Vec<String>,
    },
    Import {
        paths: Vec<String>,
    },
    Restore {
        mode: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WorkerResponse {
    pub result: Option<ActionResult>,
    pub error: Option<String>,
}
