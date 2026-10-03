mod breeze;
mod desktop;
mod explorer;
mod font_engine;
mod legacy;
mod menu;
mod menu_icon;
mod menu_policy;
mod models;
mod optimization;
mod preset_data;
mod registry;
mod repair;
mod shell_engine;
mod taskbar;
mod transaction;
mod ui_fonts;
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

#[tauri::command]
async fn load_shell() -> Result<models::ShellState, String> {
    tauri::async_runtime::spawn_blocking(shell_engine::load)
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
async fn set_tweak(id: String, enabled: bool) -> Result<ActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || worker::run(Operation::Toggle { id, enabled }))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
async fn set_menu_item(id: String, enabled: bool) -> Result<ActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || worker::run(Operation::Menu { id, enabled }))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
async fn restore_category(category: String) -> Result<ActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        worker::run(Operation::RestoreCategory { category })
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
async fn repair_system() -> Result<ActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || worker::run(Operation::Repair))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
fn repair_progress() -> Result<models::RepairProgress, String> {
    repair::progress().map_err(|e| format!("{e:#}"))
}

#[tauri::command]
async fn optimize_system(restore: bool) -> Result<ActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if restore {
            optimization::restore()
        } else {
            optimization::apply()
        }
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
async fn optimize_menu() -> Result<ActionResult, String> {
    tauri::async_runtime::spawn_blocking(|| worker::run(Operation::OptimizeMenu))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
async fn open_backup(app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let dir = font_engine::backup_root().map_err(|e| format!("无法定位备份：{e:#}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("无法打开备份目录：{e}"))?;
    app.opener()
        .open_path(dir.to_string_lossy().into_owned(), None::<&str>)
        .map_err(|e| format!("无法打开备份文件夹：{e}"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if worker::handle_cli() {
        return;
    }
    tauri::Builder::default()
        .setup(|_| {
            explorer::watch_shell();
            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            load_bootstrap,
            apply_font,
            restore_fonts,
            import_fonts,
            load_shell,
            set_tweak,
            set_menu_item,
            restore_category,
            repair_system,
            repair_progress,
            optimize_system,
            optimize_menu,
            open_backup
        ])
        .run(tauri::generate_context!())
        .expect("启动 WinGlow失败");
}
