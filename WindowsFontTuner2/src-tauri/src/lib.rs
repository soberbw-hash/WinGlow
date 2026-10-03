mod font_engine;
mod legacy;
mod models;
mod preset_data;
mod registry;
mod transaction;
mod worker;

use models::{ActionResult, BootstrapPayload, Operation};

#[tauri::command]
async fn load_bootstrap() -> Result<BootstrapPayload, String> {
    tauri::async_runtime::spawn_blocking(font_engine::load_bootstrap)
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
async fn apply_font(preset_id: String, paths: Vec<String>) -> Result<ActionResult, String> {
    preset_data::find_preset(&preset_id).map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || worker::run(Operation::Apply { preset_id, paths }))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
async fn restore_fonts(mode: String) -> Result<ActionResult, String> {
    if mode != "last" && mode != "default" {
        return Err("未知恢复操作。".into());
    }
    tauri::async_runtime::spawn_blocking(move || worker::run(Operation::Restore { mode }))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
async fn import_fonts(paths: Vec<String>) -> Result<ActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || worker::run(Operation::Import { paths }))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if worker::handle_cli() {
        return;
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            load_bootstrap,
            apply_font,
            restore_fonts,
            import_fonts
        ])
        .run(tauri::generate_context!())
        .expect("启动 Windows 微调失败");
}
