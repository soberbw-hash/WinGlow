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
    pub current_preview_weight: u16,
    pub needs_font_refresh: bool,
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
    Toggle {
        id: String,
        enabled: bool,
    },
    Menu {
        id: String,
        enabled: bool,
    },
    RestoreCategory {
        category: String,
    },
    OptimizeCore {
        token: String,
    },
    UndoOptimization {
        token: String,
    },
    OptimizeMenu,
    Appearance {
        id: String,
        settings: crate::visual::Appearance,
    },
    OptionalTool {
        id: String,
        verb: String,
    },
    Repair,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToggleState {
    pub id: String,
    pub label: String,
    pub enabled: bool,
    pub note: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MenuItem {
    pub id: String,
    pub label: String,
    pub group: String,
    pub enabled: bool,
    pub kind: String,
    pub raw_label: String,
    pub auto_hide: bool,
    pub targets: Vec<String>,
    pub menu_level: String,
    pub children: Vec<String>,
    pub sub_items: Vec<MenuItem>,
    pub visibility_note: Option<String>,
    pub icon_data_url: Option<String>,
    pub icon_source: Option<String>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellState {
    pub items: Vec<MenuItem>,
    pub tweaks: Vec<ToggleState>,
    pub breeze_enabled: bool,
    pub taskbar_enabled: bool,
    pub taskbar_supported: bool,
    pub start_menu_enabled: bool,
    pub start_menu_supported: bool,
    pub window_material_enabled: bool,
    pub window_material_supported: bool,
    pub appearance: crate::visual::Appearance,
    pub optional_tools: Vec<crate::optional_tools::ToolState>,
    pub optimization_active: bool,
    pub optimization_pending: bool,
    pub pending_recovery: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RepairProgress {
    pub running: bool,
    pub stage: String,
    pub log_dir: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WorkerResponse {
    pub result: Option<ActionResult>,
    pub error: Option<String>,
}
